use std::{collections::HashMap, fmt::Display, process::Command, str::FromStr};

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

#[derive(PartialEq, Debug, Clone)]
enum PackageSize {
    Bytes(f32),
    KiB(f32), // rough size given
    MiB(f32),
    // GiB(f32),  // are there any packages on this scale?
}

impl Default for PackageSize {
    fn default() -> Self {
        Self::Bytes(0.0)
    }
}

impl PartialOrd for PackageSize {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            // equal ranks
            (PackageSize::Bytes(s1), PackageSize::Bytes(s2)) => s1.partial_cmp(s2),
            (PackageSize::KiB(s1), PackageSize::KiB(s2)) => s1.partial_cmp(s2),
            (PackageSize::MiB(s1), PackageSize::MiB(s2)) => s1.partial_cmp(s2),
            // first is smaller rank
            (PackageSize::Bytes(s1), PackageSize::KiB(s2)) => s1.partial_cmp(&(s2 * 1024.0)),
            (PackageSize::Bytes(s1), PackageSize::MiB(s2)) => {
                s1.partial_cmp(&(s2 * 1024.0 * 1024.0))
            }
            (PackageSize::KiB(s1), PackageSize::MiB(s2)) => s1.partial_cmp(&(s2 * 1024.0)),
            // first is larger rank
            (s, o) => o.partial_cmp(s).map(|o| o.reverse()),
        }
    }
}

#[test]
fn test_package_size_ordering() {
    assert!(PackageSize::Bytes(1.0) < PackageSize::KiB(1.0));
    assert!(PackageSize::Bytes(1.0) < PackageSize::MiB(1.0));
    assert!(PackageSize::Bytes(1025.0) > PackageSize::KiB(1.0));
    assert!(PackageSize::KiB(1.0) < PackageSize::Bytes(1025.0));
    assert!(PackageSize::KiB(1.0) > PackageSize::Bytes(1.0));
    assert!(PackageSize::KiB(1.0) < PackageSize::MiB(1.0));
    assert!(PackageSize::KiB(1025.0) > PackageSize::MiB(1.0));
    assert!(PackageSize::MiB(1.0) < PackageSize::KiB(1025.0));
    assert!(PackageSize::MiB(1.0) > PackageSize::Bytes(1.0));
    assert!(PackageSize::MiB(1.0) > PackageSize::KiB(1.0));
}

impl FromStr for PackageSize {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut sp = s.split_whitespace();
        let size = sp.next().ok_or(())?;
        let rank = sp.next().ok_or(())?;
        if rank == "B" {
            let size: f32 = size.parse().map_err(|_| ())?;
            return Ok(PackageSize::Bytes(size));
        }
        if rank == "KiB" {
            let size: f32 = size.parse().map_err(|_| ())?;
            return Ok(PackageSize::KiB(size));
        }
        if rank == "MiB" {
            let size: f32 = size.parse().map_err(|_| ())?;
            return Ok(PackageSize::MiB(size));
        }
        Err(())
    }
}

impl EguiInspect for PackageSize {
    fn inspect(&self, label: &str, ui: &mut egui::Ui) {
        ui.label(format!("{label}: {}", self.to_string()));
    }
}

impl Display for PackageSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = match self {
            PackageSize::Bytes(s) => format!("{s} Bytes"),
            PackageSize::KiB(s) => format!("{s} KiB"),
            PackageSize::MiB(s) => format!("{s} MiB"),
        };
        f.write_str(data.as_str())
    }
}

// NOTE: Clone added as Graph requirement
#[derive(Debug, EguiInspect, Default, Clone)]
pub struct PackageInfo {
    pub depends: Vec<PackageName>,
    optional: Vec<OptionalDep>,
    required_by: Vec<PackageName>,
    size: PackageSize,
    other: HashMap<String, String>,
}

/// space separated string vec
fn sssv(s: String) -> Vec<PackageName> {
    s.split_whitespace()
        .map(|s| String::from(s).into())
        .collect()
}

pub fn pacman_queery(name: &str) -> Option<(String, PackageInfo)> {
    let out = Command::new("pacman").arg("-Qi").arg(name).output();
    match out {
        Ok(o) => {
            let package_info = String::from_utf8(o.stdout).unwrap();
            if package_info.is_empty() || &package_info[..=6] == "error:" {
                None
            } else {
                Some(PackageInfo::parse(&package_info))
            }
        }
        Err(e) => {
            dbg!(e);
            None
        }
    }
}

pub fn pacman_queery_all() -> HashMap<String, PackageInfo> {
    let mut res = HashMap::new();
    let out = Command::new("pacman").arg("-Qi").output().unwrap();
    let packages_str = String::from_utf8(out.stdout).unwrap();
    for pkg_str in packages_str.trim().split("\n\n") {
        let (name, pi) = PackageInfo::parse(pkg_str);
        res.insert(name, pi);
    }
    res
}

impl PackageInfo {
    fn parse(s: &str) -> (String, Self) {
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
        let size = other
            .remove("Installed Size")
            .and_then(|s| s.parse().ok())
            .expect(failure.as_str());

        (
            name,
            Self {
                depends,
                optional,
                required_by,
                size,
                other,
            },
        )
    }
}
