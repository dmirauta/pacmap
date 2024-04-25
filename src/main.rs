use std::{cell::RefCell, collections::HashMap, process::Command};

use egui_inspect::{
    quick_app::{IntoApp, QuickApp},
    EguiInspect,
};

thread_local! {
    static PACKAGE_INFOS: RefCell<HashMap<String, PackageInfo>> = Default::default();
    static NEXT: RefCell<Option<String>> = Default::default();
}

#[derive(Debug, Default)]
struct PackageName(String);

impl From<String> for PackageName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl EguiInspect for PackageName {
    fn inspect(&self, _label: &str, ui: &mut egui::Ui) {
        if ui.button(self.0.as_str()).clicked() {
            NEXT.with_borrow_mut(|n| *n = Some(self.0.clone()));
        }
    }

    fn inspect_mut(&mut self, _label: &str, _ui: &mut egui::Ui) {
        todo!()
    }
}

#[derive(Debug, EguiInspect, Default)]
struct PackageInfo {
    depends: Vec<PackageName>,
    required_by: Vec<PackageName>,
    other: HashMap<String, String>,
}

/// space separated string vec
fn sssv(s: String) -> Vec<PackageName> {
    s.split_whitespace()
        .map(|s| String::from(s).into())
        .collect()
}

fn pacman_queery(name: &str) -> Option<(String, PackageInfo)> {
    let out = Command::new("pacman").arg("-Qi").arg(name).output();
    out.map(|o| {
        let package_info = String::from_utf8(o.stdout).unwrap();
        PackageInfo::parse(package_info)
    })
    .map_err(|e| dbg!(e))
    .ok()
}

impl PackageInfo {
    fn parse(s: String) -> (String, Self) {
        let mut other: HashMap<String, String> = s
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| {
                let mut sp = l.split(" : ");
                (
                    sp.next().unwrap().trim().into(),
                    sp.next().unwrap().trim().into(),
                )
            })
            .collect();

        let name = other.remove("Name").unwrap();
        let depends = sssv(other.remove("Depends On").unwrap());
        let required_by = sssv(other.remove("Required By").unwrap());

        (
            name,
            Self {
                depends,
                required_by,
                other,
            },
        )
    }
}

struct Pacmap {
    current: String,
}

impl Default for Pacmap {
    fn default() -> Self {
        PACKAGE_INFOS.with_borrow_mut(|pis| {
            if !pis.contains_key("pacman") {
                if let Some((name, pi)) = pacman_queery("pacman") {
                    pis.insert(name, pi);
                }
            }
        });

        Self {
            current: "pacman".into(),
        }
    }
}

impl EguiInspect for Pacmap {
    fn inspect(&self, _label: &str, _ui: &mut egui::Ui) {
        todo!()
    }

    fn inspect_mut(&mut self, _label: &str, ui: &mut egui::Ui) {
        PACKAGE_INFOS.with_borrow_mut(|pis| {
            pis.get(&self.current)
                .map(|pi| pi.inspect(self.current.as_str(), ui));

            if let Some(next_s) = NEXT.with_borrow_mut(|n| n.take()) {
                if !pis.contains_key(&next_s) {
                    if let Some((name, pi)) = pacman_queery(next_s.as_str()) {
                        pis.insert(name, pi);
                    }
                }

                self.current = next_s;
            }
        });
    }
}

impl IntoApp for Pacmap {}

fn main() -> eframe::Result<()> {
    QuickApp::<Pacmap>::run()
}
