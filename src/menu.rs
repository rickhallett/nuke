//! `nuke menu`: a status-bar item with a Quit All button and a tickable
//! keep list, built on plain AppKit via objc2. Shares the CLI's config.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashSet;
use std::process::Command;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSControlStateValueOff, NSControlStateValueOn,
    NSImage, NSMenu, NSMenuDelegate, NSMenuItem, NSStatusBar, NSStatusItem,
    NSVariableStatusItemLength,
};
use objc2_foundation::{NSData, NSObject, NSObjectProtocol, NSSize, NSString, ns_string};

use crate::apps::{self, App};
use crate::config::{self, Config};
use crate::plan::Plan;

#[derive(Default)]
struct Ivars {
    status_item: OnceCell<Retained<NSStatusItem>>,
    /// Apps shown in the menu the last time it was opened; item tags index this.
    apps: RefCell<Vec<App>>,
    keep: RefCell<Vec<String>>,
    include_accessory: Cell<bool>,
    ancestors: HashSet<i32>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; Controller has no Drop impl.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    struct Controller;

    unsafe impl NSObjectProtocol for Controller {}

    unsafe impl NSMenuDelegate for Controller {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            self.rebuild(menu);
        }
    }

    impl Controller {
        #[unsafe(method(quitAll:))]
        fn quit_all(&self, _sender: Option<&AnyObject>) {
            self.nuke(false);
        }

        #[unsafe(method(forceQuitAll:))]
        fn force_quit_all(&self, _sender: Option<&AnyObject>) {
            self.nuke(true);
        }

        #[unsafe(method(toggleKeep:))]
        fn toggle_keep(&self, sender: &NSMenuItem) {
            self.reload();
            let idx = sender.tag() as usize;
            let key = {
                let apps = self.ivars().apps.borrow();
                let Some(app) = apps.get(idx) else { return };
                config::keep_key(app)
            };
            let mut keep = self.ivars().keep.borrow_mut();
            match keep.iter().position(|k| k.eq_ignore_ascii_case(&key)) {
                Some(i) => {
                    keep.remove(i);
                }
                None => keep.push(key),
            }
            report(config::set("keep", config::keep_array(&keep)));
        }

        #[unsafe(method(toggleAccessory:))]
        fn toggle_accessory(&self, _sender: Option<&AnyObject>) {
            self.reload();
            let v = !self.ivars().include_accessory.get();
            self.ivars().include_accessory.set(v);
            report(config::set("include_accessory", toml_edit::value(v)));
        }

        #[unsafe(method(openConfig:))]
        fn open_config(&self, _sender: Option<&AnyObject>) {
            let Some(path) = config::path() else { return };
            if !path.exists() {
                // Seed the file so the editor has something to open.
                report(config::set("keep", config::keep_array(&self.ivars().keep.borrow())));
            }
            report(Command::new("open").arg("-t").arg(&path).status().map(drop).map_err(|e| e.to_string()));
        }

        #[unsafe(method(quitSelf:))]
        fn quit_self(&self, _sender: Option<&AnyObject>) {
            NSApplication::sharedApplication(self.mtm()).terminate(None);
        }
    }
);

impl Controller {
    fn new(mtm: MainThreadMarker, cfg: Config) -> Retained<Self> {
        let ivars = Ivars {
            keep: RefCell::new(cfg.keep),
            include_accessory: Cell::new(cfg.include_accessory),
            ancestors: apps::ancestor_pids(),
            ..Default::default()
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: NSObject's init takes no arguments and returns an instance.
        unsafe { msg_send![super(this), init] }
    }

    /// Re-read the config file so edits made outside the menu (Edit Config…,
    /// the CLI, a text editor) are never clobbered by the next tick.
    fn reload(&self) {
        match config::load() {
            Ok(cfg) => {
                *self.ivars().keep.borrow_mut() = cfg.keep;
                self.ivars().include_accessory.set(cfg.include_accessory);
            }
            Err(e) => eprintln!("nuke: {e} (keeping last known settings)"),
        }
    }

    fn plan<'a>(&'a self, keep: &'a [String]) -> Plan<'a> {
        Plan {
            keep: keep.iter().map(String::as_str).collect(),
            only: &[],
            include_accessory: self.ivars().include_accessory.get(),
            ancestors: &self.ivars().ancestors,
        }
    }

    fn nuke(&self, force: bool) {
        self.reload();
        let keep = self.ivars().keep.borrow();
        let plan = self.plan(&keep);
        for app in apps::running().iter().filter(|a| plan.targets(a)) {
            let ok = if force {
                app.force_terminate()
            } else {
                app.terminate()
            };
            if !ok {
                eprintln!("nuke: could not send quit to {}", app.name);
            }
        }
    }

    fn item(
        &self,
        title: &str,
        action: Option<objc2::runtime::Sel>,
        key: &str,
    ) -> Retained<NSMenuItem> {
        let mtm = self.mtm();
        // SAFETY: the selector, when given, names a method defined on Controller above.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                action,
                &NSString::from_str(key),
            )
        };
        if action.is_some() {
            // SAFETY: self outlives the menu (both live until the app exits).
            unsafe { item.setTarget(Some(self)) };
        }
        item
    }

    fn rebuild(&self, menu: &NSMenu) {
        let mtm = self.mtm();
        self.reload();
        menu.removeAllItems();

        let all = apps::running();
        let keep = self.ivars().keep.borrow();
        let plan = self.plan(&keep);
        let listed: Vec<App> = all.into_iter().filter(|a| plan.candidate(a)).collect();
        let doomed = listed.iter().filter(|a| plan.targets(a)).count();

        let quit = self.item(&format!("Quit All ({doomed})"), Some(sel!(quitAll:)), "");
        quit.setEnabled(doomed > 0);
        menu.addItem(&quit);
        let force = self.item("Force Quit All", Some(sel!(forceQuitAll:)), "");
        force.setEnabled(doomed > 0);
        menu.addItem(&force);
        menu.addItem(&NSMenuItem::separatorItem(mtm));

        let header = self.item("Tick an app to keep it running", None, "");
        header.setEnabled(false);
        menu.addItem(&header);
        for (i, app) in listed.iter().enumerate() {
            let item = self.item(&app.name, Some(sel!(toggleKeep:)), "");
            item.setTag(i as isize);
            let kept = !plan.targets(app);
            item.setState(if kept {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
            if let Some(icon) = app.icon() {
                icon.setSize(NSSize::new(16.0, 16.0));
                item.setImage(Some(&icon));
            }
            menu.addItem(&item);
        }
        drop(keep);
        *self.ivars().apps.borrow_mut() = listed;

        menu.addItem(&NSMenuItem::separatorItem(mtm));
        let acc = self.item("Include Menu Bar Apps", Some(sel!(toggleAccessory:)), "");
        acc.setState(if self.ivars().include_accessory.get() {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        menu.addItem(&acc);
        menu.addItem(&self.item("Edit Config…", Some(sel!(openConfig:)), ""));
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&self.item("Quit nuke", Some(sel!(quitSelf:)), "q"));
    }
}

/// A tiny mushroom cloud. Vector, so it's crisp at any scale; a template
/// image, so it follows the menu bar's light/dark appearance.
fn menubar_icon(_mtm: MainThreadMarker) -> Option<Retained<NSImage>> {
    const SVG: &[u8] = include_bytes!("../assets/menubar.svg");
    let img = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(SVG))?;
    img.setSize(NSSize::new(18.0, 18.0));
    img.setTemplate(true);
    Some(img)
}

fn report(r: Result<(), String>) {
    if let Err(e) = r {
        eprintln!("nuke: {e}");
    }
}

pub fn run(cfg: Config) {
    let mtm = MainThreadMarker::new().expect("nuke menu must start on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    // Accessory: menu bar presence only, no Dock icon, no app menu.
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let controller = Controller::new(mtm, cfg);

    let status_item =
        NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
    if let Some(button) = status_item.button(mtm) {
        match menubar_icon(mtm) {
            Some(img) => button.setImage(Some(&img)),
            None => button.setTitle(ns_string!("☢")),
        }
        button.setToolTip(Some(ns_string!("nuke: quit all apps")));
    }

    let menu = NSMenu::new(mtm);
    menu.setDelegate(Some(ProtocolObject::from_ref(&*controller)));
    status_item.setMenu(Some(&menu));
    controller.ivars().status_item.set(status_item).ok();

    app.run();
}
