extends MeshInstance3D

var original_translation := Vector3()
var original_euler_angles := Vector3()
var original_scale := Vector3(1.0, 1.0, 1.0)
var original_color := Color(1.0, 1.0, 1.0, 1.0)

var velocity := Vector3()
var angular_velocity := Vector3()
var scale_velocity := Vector3()
var color_velocity := Vector3()

var acceleration := Vector3()

var sinusoidal_translation_amplitude := Vector3()
var sinusoidal_translation_speed := Vector3()

var animate_color := false
var sinusoidal_color_amplitude := Vector3()
var sinusoidal_color_speed := Vector3()

var is_spawner := false
var spawn_interval := 0.0
var time_until_next_spawn := 0.0

var is_ephemeral := false
var ttl := 0.0

var needs_unique_material := false

func _ready():
	#if basis.z.is_zero_approx() or basis.y.is_zero_approx() or basis.x.is_zero_approx():
		#print("Basis is 0 for %s, scale = %s" % [self.name, self.scale])
	var quat = quaternion
	rotation_order = EULER_ORDER_XYZ
	quaternion = quat
	original_translation = position
	original_euler_angles = rotation
	original_scale = scale
	process_mode = Node.PROCESS_MODE_DISABLED
	if has_meta("extras"):
		var extras = get_meta("extras")
		var animations = extras.get("animations")
		if animations != null:
			for anim in animations:
				_add_animation(anim)
	_setup_blend_mode()
	# print("Initialized!", self)

func _add_animation(dict):
	match dict:
		{ "type": "spawner", "interval": var interval, "ttl": var field_ttl }:
			if field_ttl > 0:
				if not is_ephemeral:
					# This is a spawner
					is_spawner = true
					spawn_interval = interval / 60.0
					time_until_next_spawn = 0.0
					visible = false
					process_mode = Node.PROCESS_MODE_INHERIT
				else:
					# This was spawned
					ttl = field_ttl / 60.0
				
		{ "type": "velocity", "value": [var x, var y, var z] }:
			velocity = Vector3(x, y, z)
			if not velocity.is_zero_approx():
				process_mode = Node.PROCESS_MODE_INHERIT
		{ "type": "angular_velocity", "value": [var x, var y, var z] }:
			angular_velocity = Vector3(x, y, z)
			if not angular_velocity.is_zero_approx():
				process_mode = Node.PROCESS_MODE_INHERIT
		{ "type": "scale_velocity", "value": [var x, var y, var z] }:
			scale_velocity = Vector3(x, y, z)
			if not scale_velocity.is_zero_approx():
				process_mode = Node.PROCESS_MODE_INHERIT
		{ "type": "color_velocity", "value": [var x, var y, var z] }:
			color_velocity = Vector3(x, y, z)
			if not color_velocity.is_zero_approx():
				animate_color = true
				needs_unique_material = true
				process_mode = Node.PROCESS_MODE_INHERIT
				
		{ "type": "acceleration", "value": [var x, var y, var z] }:
			acceleration = Vector3(x, y, z)
			if not acceleration.is_zero_approx():
				process_mode = Node.PROCESS_MODE_INHERIT
				
		{ 
			"type": "sinusoidal_translation",
			"amplitude": [var amp_x, var amp_y, var amp_z],
			"speed": [var speed_x, var speed_y, var speed_z]
		}:
			sinusoidal_translation_amplitude = Vector3(amp_x, amp_y, amp_z)
			sinusoidal_translation_speed = Vector3(speed_x, speed_y, speed_z) * TAU / 72.0
			if not (sinusoidal_translation_amplitude.is_zero_approx() or sinusoidal_translation_speed.is_zero_approx()):
				process_mode = Node.PROCESS_MODE_INHERIT
		{ 
			"type": "sinusoidal_color",
			"amplitude": [var amp_x, var amp_y, var amp_z],
			"speed": [var speed_x, var speed_y, var speed_z]
		}:
			sinusoidal_color_amplitude = Vector3(amp_x, amp_y, amp_z)
			sinusoidal_color_speed = Vector3(speed_x, speed_y, speed_z) * TAU / 72.0
			if not (sinusoidal_color_amplitude.is_zero_approx() or sinusoidal_color_speed.is_zero_approx()):
				animate_color = true
				needs_unique_material = true
				process_mode = Node.PROCESS_MODE_INHERIT
		var unknown:
			push_error("Unknown animation type: ", unknown)
	
var surface_original_color = []
func _setup_blend_mode():
	var albedo_mult = null
	var blend_override = null
	if has_meta("extras"):
		var extras = get_meta("extras")
		match extras.get("godot_tint"):
			[var r, var g, var b]:
				albedo_mult = Color(r, g, b)
				original_color = albedo_mult
		blend_override = extras.get("godot_blend_override")
	needs_unique_material = needs_unique_material or albedo_mult != null or (blend_override != null and blend_override != "none")
	if needs_unique_material:
		mesh = mesh.duplicate()
	for surface_idx in mesh.get_surface_count():
		var material: StandardMaterial3D = mesh.surface_get_material(surface_idx)
		if needs_unique_material:
			var mat = material.duplicate()
			mesh.surface_set_material(surface_idx, mat)
			material = mat
		match blend_override:
			"opaque":
				material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA_SCISSOR
				material.blend_mode = BaseMaterial3D.BLEND_MODE_MIX
				material.albedo_color = Color(1.0, 1.0, 1.0, 1.0)
			"mix":
				material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
				material.blend_mode = BaseMaterial3D.BLEND_MODE_MIX
				material.albedo_color = Color(1.0, 1.0, 1.0, 0.5)
			"add":
				material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA_SCISSOR
				material.blend_mode = BaseMaterial3D.BLEND_MODE_ADD
				material.albedo_color = Color(1.0, 1.0, 1.0, 1.0)
			"subtract":
				material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA_SCISSOR
				material.blend_mode = BaseMaterial3D.BLEND_MODE_SUB
				material.albedo_color = Color(1.0, 1.0, 1.0, 1.0)
			"quarter_add":
				material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA_SCISSOR
				material.blend_mode = BaseMaterial3D.BLEND_MODE_ADD
				material.albedo_color = Color(0.25, 0.25, 0.25, 1.0)
			"none":
				pass
			null:
				pass
			var unknown:
				if not is_ephemeral:
					push_error("Unknown blend_override in %s: %s" % [name, unknown])
		surface_original_color.push_back(material.albedo_color)
		if albedo_mult != null:
			material.albedo_color *= albedo_mult
		if material.transparency == BaseMaterial3D.TRANSPARENCY_ALPHA_SCISSOR:
			# Alpha to coverage doesn't seem to work right, at all. It has a thin opaque ring edge artifact, and enables blending??
			# Might be from compression?
			#material.alpha_antialiasing_mode = BaseMaterial3D.ALPHA_ANTIALIASING_ALPHA_TO_COVERAGE
			pass
			
func _set_color(c: Color):
	for surface_idx in mesh.get_surface_count():
		var material: StandardMaterial3D = mesh.surface_get_material(surface_idx)
		var s_color = surface_original_color[surface_idx]
		material.albedo_color = s_color * c
		
var elapsed := 0.0
var spawn_counter := 0
func _process(delta: float) -> void:
	elapsed += delta
	
	if is_spawner:
		time_until_next_spawn -= delta
		while time_until_next_spawn < 0:
			time_until_next_spawn += spawn_interval
			var spawned = duplicate(Node.DUPLICATE_SCRIPTS)
			spawned.is_ephemeral = true
			spawned.visible = true
			spawned.name = "%s #%s" % [name, spawn_counter]
			spawn_counter += 1
			get_parent().add_child(spawned)
		return
		
	if is_ephemeral:
		ttl -= delta
		if ttl < 0.0:
			queue_free()
	
	scale = original_scale + elapsed * scale_velocity
	rotation = original_euler_angles + elapsed * angular_velocity * 30.0
	
	var phase := elapsed * sinusoidal_translation_speed
	var sin_trans := Vector3(sin(phase.x), sin(phase.y), sin(phase.z))
	position = (
		original_translation
		+ elapsed * (velocity + elapsed * 0.5 * acceleration)
		+ sinusoidal_translation_amplitude * sin_trans
	)
	
	if animate_color:
		var color_phase := elapsed * sinusoidal_color_speed
		var color_phase_sin := Vector3(sin(color_phase.x), sin(color_phase.y), sin(color_phase.z))
		var color_offset :=  color_velocity * elapsed + sinusoidal_color_amplitude * color_phase_sin
		var color_vec = Vector3(original_color.r, original_color.g, original_color.b) + color_offset
		color_vec = color_vec.clampf(0.0, 1.0)
		_set_color(Color(color_vec.x, color_vec.y, color_vec.z))
