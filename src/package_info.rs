use std::{collections::HashMap, process::Command, str::FromStr};

use egui_inspect::{egui, EguiInspect};

use crate::NEXT;

#[derive(Debug, Default, Clone)]
pub struct PackageName(pub String);

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

#[derive(EguiInspect, Default, Debug, Clone)]
pub struct OptionalDep {
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

// NOTE: Clone added as Graph requirement
#[derive(Debug, EguiInspect, Default, Clone)]
pub struct PackageInfo {
    pub depends: Vec<PackageName>,
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

pub fn pacman_queery(name: impl AsRef<str>) -> Option<(String, PackageInfo)> {
    let out = Command::new("pacman")
        .arg("-Qi")
        .arg(name.as_ref())
        .output();
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
                if let Ok(pi) = k.parse() {
                    optional.push(pi);
                }
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
