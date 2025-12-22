//! SensorId - Cheap-to-clone sensor identifier
//!
//! Uses Arc<str> internally for O(1) clone operations.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

/// Sensor identifier with cheap cloning.
///
/// Internally uses `Arc<str>` so cloning only increments a reference count
/// instead of allocating new memory. This is ideal for sensor IDs that are
/// created once at configuration time and cloned frequently during runtime.
///
/// # Examples
/// ```
/// use contracts::SensorId;
///
/// let id: SensorId = "front_camera".into();
/// let id2 = id.clone();  // O(1) - just increments ref count
/// assert_eq!(id, id2);
/// assert_eq!(id.as_str(), "front_camera");
/// ```
#[derive(Clone, Default)]
pub struct SensorId(Arc<str>);

impl SensorId {
    /// Create a new SensorId from a string slice.
    #[inline]
    pub fn new(s: &str) -> Self {
        Self(Arc::from(s))
    }

    /// Get the underlying string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Deref to &str for easy string operations
impl Deref for SensorId {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for SensorId {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for SensorId {
    #[inline]
    fn borrow(&self) -> &str {
        &self.0
    }
}

// Conversions
impl From<&str> for SensorId {
    #[inline]
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

impl From<String> for SensorId {
    #[inline]
    fn from(s: String) -> Self {
        Self(Arc::from(s))
    }
}

impl From<Arc<str>> for SensorId {
    #[inline]
    fn from(s: Arc<str>) -> Self {
        Self(s)
    }
}

// Display and Debug
impl fmt::Display for SensorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for SensorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SensorId({:?})", self.0)
    }
}

// Equality - can compare with &str, String, etc.
impl PartialEq for SensorId {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // Fast path: same Arc pointer
        Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
    }
}

impl Eq for SensorId {}

impl PartialEq<str> for SensorId {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.0.as_ref() == other
    }
}

impl PartialEq<&str> for SensorId {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.0.as_ref() == *other
    }
}

impl PartialEq<String> for SensorId {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.0.as_ref() == other
    }
}

// Hash - same as str hash for HashMap compatibility
impl Hash for SensorId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

// Serde support
impl Serialize for SensorId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SensorId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_clone_is_cheap() {
        let id1: SensorId = "test_sensor".into();
        let id2 = id1.clone();

        // Both should point to same underlying data (Arc clone is O(1))
        assert_eq!(id1.as_str().as_ptr(), id2.as_str().as_ptr());
    }

    #[test]
    fn test_equality() {
        let id: SensorId = "cam1".into();
        assert_eq!(id, "cam1");
        assert_eq!(id, String::from("cam1"));
        assert_eq!(id, SensorId::from("cam1"));
    }

    #[test]
    fn test_hashmap_key() {
        let mut map: HashMap<SensorId, i32> = HashMap::new();
        map.insert("sensor1".into(), 1);
        map.insert("sensor2".into(), 2);

        // Can lookup with &str
        assert_eq!(map.get("sensor1"), Some(&1));
        assert_eq!(map.get("sensor2"), Some(&2));
    }

    #[test]
    fn test_serde() {
        let id: SensorId = "test".into();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"test\"");

        let parsed: SensorId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }
}
