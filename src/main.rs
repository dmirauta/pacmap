use std::{cell::RefCell, collections::HashMap, process::Command, str::FromStr};

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

#[derive(EguiInspect, Default, Debug)]
struct OptionalDep {
    package_name: String,
    reason: String,
}

impl FromStr for OptionalDep {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut sp = s.split(": ");
        match (sp.next(), sp.next()) {
            (Some(package_name), Some(reason)) => Ok(Self {
                package_name: package_name.trim().to_string(),
                reason: reason.into(),
            }),
            _ => Err("Unexpected OptionalDep format".into()),
        }
    }
}

#[derive(Debug, EguiInspect, Default)]
struct PackageInfo {
    depends: Vec<PackageName>,
    optional: Vec<OptionalDep>,
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
    match out {
        Ok(o) => {
            let package_info = String::from_utf8(o.stdout).unwrap();
            if package_info.is_empty() || &package_info[..=6] == "error:" {
                None
            } else {
                Some(PackageInfo::parse(package_info))
            }
        }
        Err(e) => {
            dbg!(e);
            None
        }
    }
}

impl PackageInfo {
    fn parse(s: String) -> (String, Self) {
        let mut other = HashMap::new();
        let mut optional = vec![];

        for l in s.lines().filter(|l| !l.is_empty()) {
            let mut sp = l.split(" : ");
            let key = sp.next();
            let val = sp.next();
            if let Some(v) = val {
                let k: String = key.unwrap().into();
                if k != "Optional Deps" {
                    other.insert(k.trim().into(), v.into());
                } else {
                    optional.push(v.parse().unwrap());
                }
            } else {
                let k: String = key.unwrap().into();
                optional.push(k.parse().unwrap());
            }
        }

        let failure = format!("Failed parsing {s}");
        let name = other.remove("Name").expect(failure.as_str());
        let depends = sssv(other.remove("Depends On").expect(failure.as_str()));
        let required_by = sssv(other.remove("Required By").expect(failure.as_str()));

        (
            name,
            Self {
                depends,
                optional,
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
                        self.current = next_s;
                    }
                }
            }
        });
    }
}

impl IntoApp for Pacmap {}

fn main() -> eframe::Result<()> {
    QuickApp::<Pacmap>::run()
}
