//! Volume management for persistent storage.
//!
//! This module provides volume management for services. Volumes are
//! persistent directories that can be mounted into service containers.
//! The volume manager handles:
//!
//! - Volume lifecycle (create, delete, list)
//! - Mount/unmount tracking
//! - Directory creation and cleanup

mod manager;
mod types;

pub use manager::{SharedVolumeManager, VolumeManager};
pub use types::{
    VolumeConfig, VolumeDetail, VolumeInfo, VolumeMount, VolumeState, generate_volume_id,
};
