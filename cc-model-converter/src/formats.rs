pub mod animated_model;
pub mod animations;
pub mod chunkctd;
pub mod ltc;
pub mod ltd;
pub mod prd;
pub mod camera_path;
pub mod meshinstance;
pub mod cpt;
pub mod tim;
pub mod weapbin;
pub mod mapbin;
pub mod mapctd;
pub mod lzss;

use glam::f32::{Vec3 as Vector3, vec3, Mat4 as Matrix4, Quat as Quaternion};

const CC_SCALE: f32 = 1.0 / 625.0;
const CC_AXIS_FLIP: Vector3 = vec3(1.0, -1.0, -1.0);

fn cc2rad(rotationAmount: i16) -> f32 {
    rotationAmount as f32 * std::f32::consts::PI * 2.0 / 4096.0
}

// These three functions should be used for all conversions from CC-space, shuffle/negate axes here
fn spatial_norm(x: i8, y: i8, z: i8) -> Vector3 {
    CC_AXIS_FLIP * vec3(x as f32, y as f32, z as f32) / 127.0
}
fn spatial(x: i16, y: i16, z: i16) -> Vector3 {
    CC_AXIS_FLIP * vec3(x as f32, y as f32, z as f32) * CC_SCALE
}
pub fn euler_angles(x: i16, y: i16, z: i16) -> Vector3 {
    CC_AXIS_FLIP * vec3(cc2rad(x), cc2rad(y), cc2rad(z))
}
pub fn cc_model_euler_angles_to_quaternion(euler: Vector3) -> Quaternion {
    Quaternion::from_euler(glam::EulerRot::ZYX, euler.z, euler.y, euler.x)
}
pub fn cc_mesh_euler_angles_to_quaternion(euler: Vector3) -> Quaternion {
    Quaternion::from_euler(glam::EulerRot::XYZ, euler.x, euler.y, euler.z)
}

// Need these exposed for now, for mesh instance command parsing
pub fn cc_position(x: i16, y: i16, z: i16) -> Vector3 {
    spatial(x, y, z)
}
pub fn cc_model_quaternion(x: i16, y: i16, z: i16) -> Quaternion {
    cc_model_euler_angles_to_quaternion(euler_angles(x, y, z))
}
pub fn cc_mesh_quaternion(x: i16, y: i16, z: i16) -> Quaternion {
    cc_mesh_euler_angles_to_quaternion(euler_angles(x, y, z))
}

#[derive(binread::BinRead)]
pub struct Transform {
    dAngleX: i16,
    dAngleY: i16,
    dAngleZ: i16,

    dPositionX: i16,
    dPositionY: i16,
    dPositionZ: i16,
}

impl Transform {
    pub fn translation(&self) -> Vector3 {
        spatial(self.dPositionX, self.dPositionY, self.dPositionZ)
    }
    pub fn euler_angles(&self) -> Vector3 {
        euler_angles(self.dAngleX, self.dAngleY, self.dAngleZ)
    }
    pub fn quaternion(&self) -> Quaternion {
        cc_model_euler_angles_to_quaternion(self.euler_angles())
    }
    pub fn matrix(&self) -> Matrix4 {
        Matrix4::from_translation(self.translation()) * Matrix4::from_quat(self.quaternion())
    }
    pub fn has_nonzero_translation(&self) -> bool {
        !(self.dPositionX == 0 && self.dPositionY == 0 && self.dPositionZ == 0)
    }
    pub fn has_nonzero_rotation(&self) -> bool {
        !(self.dAngleX == 0 && self.dAngleY == 0 && self.dAngleZ == 0)
    }
}