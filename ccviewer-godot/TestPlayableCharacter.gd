extends Node3D

var walkMesh = null
var animPlayer: AnimationPlayer
var desiredRotationY := 0.0

func _ready():
	var sergeFieldModelBytes = await %Tree.generateGltf(["--format=gltf", "--zip", %Tree.cdrom_path, "fieldobj/player/selju/selju.obj"])
	if sergeFieldModelBytes != null:
		_load_model(sergeFieldModelBytes)
	
func _load_model(bytes: PackedByteArray):
	var state = GLTFState.new()
	var doc = GLTFDocument.new()
	var err = doc.append_from_buffer(bytes, "", state)
	if err != OK:
		print("err ", err, " appending to gltf from buffer")
		return
		
	var node = doc.generate_scene(state)
		
	animPlayer = node.find_child("AnimationPlayer")
	if animPlayer is AnimationPlayer:
		var animLibrary = animPlayer.get_animation_library('')
		var skeleton = node.find_child("Skeleton3D")
		animPlayer.deterministic = false
		animPlayer.playback_default_blend_time = 0.5
		for animation in Array(animPlayer.get_animation_list()):
			var anim = animLibrary.get_animation(animation)
			animLibrary.remove_animation(animation)
			if skeleton is Skeleton3D:
				# This allows deterministic = false, since blending doesn't work correctly with it on
				_add_default_tracks(animPlayer, anim, skeleton)
			animLibrary.add_animation(animation, %Preview.make_looped_version(anim))
			
	$DefaultCylinder.queue_free()
	add_child(node)
	
static func _add_default_tracks(animationPlayer: AnimationPlayer, anim: Animation, skeleton: Skeleton3D):
	var skellyPath = animationPlayer.get_node(animationPlayer.root_node).get_path_to(skeleton)
	var existingPositionTracks = {}
	var existingRotationTracks = {}
	for trackIndex in anim.get_track_count():
		match anim.track_get_type(trackIndex):
			Animation.TYPE_POSITION_3D:
				existingPositionTracks[anim.track_get_path(trackIndex)] = true
			Animation.TYPE_ROTATION_3D:
				existingRotationTracks[anim.track_get_path(trackIndex)] = true
	for boneIndex in skeleton.get_bone_count():
		var boneName = skeleton.get_bone_name(boneIndex)
		var nodePath = NodePath("%s:%s" % [skellyPath, boneName])
		if not existingPositionTracks.has(nodePath):
			var pos = skeleton.get_bone_pose_position(boneIndex)
			var posTrack = anim.add_track(Animation.TYPE_POSITION_3D)
			anim.track_set_path(posTrack, nodePath)
			anim.track_insert_key(posTrack, 0.0, pos)
		if not existingRotationTracks.has(nodePath):
			var rot = skeleton.get_bone_pose_rotation(boneIndex)
			var rotTrack = anim.add_track(Animation.TYPE_ROTATION_3D)
			anim.track_set_path(rotTrack, nodePath)
			anim.track_insert_key(rotTrack, 0.0, rot)
	
func _process(delta: float) -> void:
	var input_basis := get_viewport().get_camera_3d().global_basis
	var input_x_dir := input_basis.x.slide(Vector3.UP).normalized()
	var input_y_dir := -input_x_dir.cross(Vector3.UP)
	var input_vec := Vector2()
	var run := false
	input_vec = Input.get_vector("move_left", "move_right", "move_down", "move_up")
	run = input_vec.length() > 0.8
	if run:
		input_vec *= 1.3 / input_vec.length()
	elif not input_vec.is_zero_approx():
		input_vec *= 0.5 / input_vec.length()
	var move_dir := input_vec.x * input_x_dir + input_vec.y * input_y_dir
	var old_pos = position
	position += move_dir * delta
	if is_instance_valid(walkMesh):
		position = walkMesh.closest_point(position)
	var motion = position - old_pos
	if not motion.is_zero_approx():
		desiredRotationY = atan2(motion.x, motion.z)
	else:
		var look_vec := Input.get_vector("look_left", "look_right", "look_down", "look_up")
		var look_dir := look_vec.x * input_x_dir + look_vec.y * input_y_dir
		if not look_dir.is_zero_approx():
			desiredRotationY = atan2(look_dir.x, look_dir.z)
	rotation.y = lerp_angle(rotation.y, desiredRotationY, 1.0 - exp(delta * -30.0))
	if animPlayer != null:
		if run:
			animPlayer.play("Anim2")
		elif not move_dir.is_zero_approx():
			animPlayer.play("Anim3")
		else:
			animPlayer.play("Anim1")
