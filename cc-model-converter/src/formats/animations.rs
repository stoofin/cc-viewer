use binread::{BinRead,FilePtr32, derive_binread};
use super::Transform;
use crate::CurPos;

fn popcount(bits: &[u32]) -> u32 {
    bits.iter().cloned().map(|u| u.count_ones()).sum()
}

#[derive(BinRead, Debug, PartialEq, Eq)]
pub struct JointKeyFrame {
    x: i16,
    y: i16,
    z: i16,
}

#[derive(BinRead)]
#[br(import(numAnimatedJoints: u32))]
pub struct AnimationKeyframe {
    #[br(count = numAnimatedJoints)]
    pub jointKeyframes: Vec<JointKeyFrame>
}

#[derive_binread]
#[br(import(numTransforms: u32))]
pub struct Animation {
    #[br(temp)]
    pub baseOffset: CurPos,

    pub numFrames: u32,

    #[br(count = (numTransforms * 2 - 1) / 32 + 1)]
    pub jointBitFlags: Vec<u32>,
    #[br(calc = popcount(&jointBitFlags))]
    pub jointBitFlagsPopcount: u32,

    #[br(count = numFrames)]
    pub frameOffsets: Vec<u32>,
    #[br(temp)]
    pub keyframesPos: CurPos,
    #[br(assert(baseOffset.0 + frameOffsets[0] as u64 == keyframesPos.0, "Miscalculated start of keyframe 0? {} vs {}", baseOffset.0 + frameOffsets[0] as u64, keyframesPos.0))]
    #[br(count = numTransforms)]
    pub initialTransforms: Vec<Transform>,
    #[br(count = numFrames - 1)]
    #[br(args(jointBitFlagsPopcount))]
    pub keyframes: Vec<AnimationKeyframe>,
}

#[derive_binread]
#[br(import(numTransforms: u32))]
pub struct AnimationData {
    #[br(temp)]
    pub baseOffset: CurPos,

    pub numAnimations: u32,
    #[br(count = numAnimations)]
    #[br(offset = baseOffset.0)]
    #[br(args(numTransforms))]
    pub animations: Vec<FilePtr32<Animation>>,

    pub endOfCptOffset: u32,
}


// Methods to access data

use glam::f32::{Vec3 as Vector3, Quat as Quaternion};

impl JointKeyFrame {
    pub fn as_translation(&self) -> Vector3 {
        super::spatial(self.x, self.y, self.z)
    }
    pub fn as_euler_angles(&self) -> Vector3 {
        super::euler_angles(self.x, self.y, self.z)
    }
}

pub fn combinedRotationAsQuaternion(t: &Transform, k: &JointKeyFrame) -> Quaternion {
    // I assume adding the angles is more correct for what chrono cross was doing? As opposed to R1 * R2 as matrix composition
    // The lerping between glTF keyframes will slerp quaternions, I believe, which would also be different but hopefully not noticeable or bad.
    super::cc_model_euler_angles_to_quaternion(t.euler_angles() + k.as_euler_angles())
}
pub fn combinedTransformsAsQuaternion(a: &Transform, b: &Transform) -> Quaternion {
    super::cc_model_euler_angles_to_quaternion(a.euler_angles() + b.euler_angles())
}