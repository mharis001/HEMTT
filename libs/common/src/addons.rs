use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::{fs::DirEntry, str::FromStr};

use tracing::{trace, warn};

use crate::workspace::WorkspacePath;
use crate::{
    prefix::{self, Prefix},
    project::AddonConfig,
    version::Version,
};

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("Addon duplicated with different case: {0}")]
    AddonNameDuplicate(String),
    #[error("Addon present in addons and optionals: {0}")]
    AddonDuplicate(String),
    #[error("Invalid addon location: {0}")]
    AddonLocationInvalid(String),
    #[error("Optional addon not found: {0}")]
    AddonOptionalNotFound(String),
    #[error("Addon prefix not found: {0}")]
    AddonPrefixMissing(String),
}

#[derive(Debug, Clone)]
pub struct Addon {
    name: String,
    location: Location,
    config: Option<AddonConfig>,
    prefix: Prefix,
    build_data: BuildData,
}

impl Addon {
    /// Create a new addon
    ///
    /// # Errors
    /// - [`Error::AddonPrefixMissing`] if the prefix is missing
    /// - [`std::io::Error`] if the addon.toml file cannot be read
    /// - [`toml::de::Error`] if the addon.toml file is invalid
    /// - [`std::io::Error`] if the prefix file cannot be read
    pub fn new(root: &Path, name: String, location: Location) -> Result<Self, crate::error::Error> {
        let path = root.join(location.to_string()).join(&name);
        Ok(Self {
            config: {
                let path = path.join("addon.toml");
                if path.exists() {
                    Some(AddonConfig::from_file(&path)?)
                } else {
                    None
                }
            },
            prefix: {
                let mut prefix = None;
                let mut files = prefix::FILES
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>();
                files.append(
                    &mut prefix::FILES
                        .iter()
                        .map(|f| f.to_uppercase())
                        .collect::<Vec<String>>(),
                );
                'search: for file in &files {
                    let path = path.join(file);
                    if path.exists() {
                        let content = std::fs::read_to_string(path)?;
                        prefix = Some(Prefix::new(&content)?);
                        break 'search;
                    }
                }
                prefix.ok_or_else(|| Error::AddonPrefixMissing(name.clone()))?
            },
            location,
            name,
            build_data: BuildData::new(),
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn pbo_name(&self, prefix: &str) -> String {
        format!("{prefix}_{}", self.name)
    }

    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }

    #[must_use]
    pub const fn prefix(&self) -> &Prefix {
        &self.prefix
    }

    #[must_use]
    /// addons/foobar
    /// optionals/foobar
    pub fn folder(&self) -> String {
        format!("{}/{}", self.location.to_string(), self.name)
    }

    #[must_use]
    pub const fn config(&self) -> Option<&AddonConfig> {
        self.config.as_ref()
    }

    #[must_use]
    pub const fn build_data(&self) -> &BuildData {
        &self.build_data
    }

    /// Scan for addons in both locations
    ///
    /// # Errors
    /// - [`Error::AddonLocationInvalid`] if a location is invalid
    /// - [`Error::AddonLocationInvalid`] if a folder name is invalid
    pub fn scan(root: &Path) -> Result<Vec<Self>, crate::error::Error> {
        let mut addons = Vec::new();
        for location in [Location::Addons, Location::Optionals] {
            if let Some(scanned) = location.scan(root)? {
                addons.extend(scanned);
            }
        }
        for addon in &addons {
            if addon.name().to_lowercase() != addon.name() {
                warn!(
                    "Addon name {} is not lowercase, it is highly recommended to use lowercase names",
                    addon.name()
                );
            }
            if addons.iter().any(|a| {
                a.name().to_lowercase() == addon.name().to_lowercase() && a.name() != addon.name()
            }) {
                return Err(crate::error::Error::Addon(Error::AddonNameDuplicate(
                    addon.name().to_string(),
                )));
            }
            if addons.iter().any(|a| {
                a.name().to_lowercase() == addon.name().to_lowercase()
                    && a.location() != addon.location()
            }) {
                return Err(crate::error::Error::Addon(Error::AddonDuplicate(
                    addon.name().to_string(),
                )));
            }
        }
        Ok(addons)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    Addons,
    Optionals,
}

impl Location {
    /// Scan for addons in this location
    ///
    /// # Errors
    /// - [`Error::AddonLocationInvalid`] if a folder name is invalid
    pub fn scan(self, root: &Path) -> Result<Option<Vec<Addon>>, crate::error::Error> {
        let folder = root.join(self.to_string());
        if !folder.exists() {
            return Ok(None);
        }
        trace!("Scanning {} for addons", folder.display());
        std::fs::read_dir(folder)?
            .collect::<std::io::Result<Vec<DirEntry>>>()?
            .iter()
            .map(std::fs::DirEntry::path)
            .filter(|file_or_dir| file_or_dir.is_dir())
            .map(|file| {
                let Some(name) = file.file_name() else {
                    return Err(crate::error::Error::Addon(Error::AddonLocationInvalid(
                        file.display().to_string(),
                    )));
                };
                let Some(name) = name.to_str() else {
                    return Err(crate::error::Error::Addon(Error::AddonLocationInvalid(
                        file.display().to_string(),
                    )));
                };
                Addon::new(root, name.to_string(), self)
            })
            .collect::<Result<Vec<Addon>, crate::error::Error>>()
            .map(Some)
    }
}

impl FromStr for Location {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "addons" => Ok(Self::Addons),
            "optionals" => Ok(Self::Optionals),
            _ => Err(Error::AddonLocationInvalid(s.to_string())),
        }
    }
}

impl ToString for Location {
    fn to_string(&self) -> String {
        match self {
            Self::Addons => "addons",
            Self::Optionals => "optionals",
        }
        .to_string()
    }
}

type RequiredVersion = (Version, WorkspacePath, Range<usize>);

#[derive(Debug, Clone, Default)]
pub struct BuildData {
    required_version: Arc<RwLock<Option<RequiredVersion>>>,
}

impl BuildData {
    #[must_use]
    pub fn new() -> Self {
        Self {
            required_version: Arc::new(RwLock::new(None)),
        }
    }

    #[must_use]
    /// Fetches the required version
    ///
    /// Does not lock, the value is only accurate at the time of calling
    /// but it shouldn't change during normal HEMTT usage
    ///
    /// # Panics
    /// Panics if the lock is poisoned
    pub fn required_version(&self) -> Option<RequiredVersion> {
        self.required_version.read().unwrap().clone()
    }

    /// Sets the required version
    ///
    /// # Panics
    /// Panics if the lock is poisoned
    pub fn set_required_version(&self, version: Version, file: WorkspacePath, line: Range<usize>) {
        *self.required_version.write().unwrap() = Some((version, file, line));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    pub fn location_from_str() {
        assert_eq!("addons".parse(), Ok(super::Location::Addons));
        assert_eq!("optionals".parse(), Ok(super::Location::Optionals));
        assert_eq!(
            "foobar".parse::<super::Location>(),
            Err(super::Error::AddonLocationInvalid("foobar".to_string()))
        );
    }
}
