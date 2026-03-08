extends Node3D

var anim_t = 0.0
var camera: Camera3D
var pos_s = []
var focus_s = []

var pos_curve: Curve3D
var focus_curve: Curve3D

var duration_seconds := 2.0

func _ready():
	process_mode = Node.PROCESS_MODE_DISABLED
	_setup_paths()
	var container = get_tree().root.find_child("TempControls", true, false)
	if container != null:
		var button := Button.new()
		button.text = "Play %s (%s)" % [name, pos_s.size()]
		button.pressed.connect(self._play_camera)
		container.add_child(button)

static func _curve_point(curve: Curve3D, index: int):
	if index >= 0 and index < curve.point_count:
		return curve.get_point_position(index)
	return null

static func _catmull_rom_ify(curve: Curve3D):
	var k = 1.0/6.0
	for i in range(0, curve.point_count):
		var tangent: Vector3
		match [_curve_point(curve, i - 1), _curve_point(curve, i + 1)]:
			[null, null]:
				tangent = Vector3(0, 0, 0)
			[null, var next]:
				tangent = 2 * k * (next - curve.get_point_position(i)) # I don't think this is right
			[var prev, null]:
				tangent = 2 * k * (curve.get_point_position(i) - prev) # I don't think this is right
			[var prev, var next]:
				tangent = k * (next - prev)
		curve.set_point_out(i, tangent)
		curve.set_point_in(i, -tangent)

func _setup_paths():
	var pairs: Array = get_meta("extras", {}).get("point_pairs", [])
	for pair in pairs:
		match pair:
			{"position": [var px, var py, var pz], "focus": [var fx, var fy, var fz]}:
				pos_s.push_back(Vector3(px, py, pz))
				focus_s.push_back(Vector3(fx, fy, fz))
			var bad:
				push_error("Incorrectly formatted camera path pair: ", bad)
	duration_seconds = get_meta("extras")["camera_anim_duration"] / 60.0
	
	var pos_path := Path3D.new()
	pos_curve = Curve3D.new()
	var focus_path := Path3D.new()
	focus_curve = Curve3D.new()
	for p in pos_s: pos_curve.add_point(p)
	for p in focus_s: focus_curve.add_point(p)
	_catmull_rom_ify(pos_curve)
	_catmull_rom_ify(focus_curve)
	pos_path.curve = pos_curve
	focus_path.curve = focus_curve
	pos_path.debug_custom_color = Color("yellow")
	focus_path.debug_custom_color = Color("blue")
	pos_path.name = "PosPath"
	focus_path.name = "FocusPath"
	add_child(pos_path)
	add_child(focus_path)
	
	camera = Camera3D.new()
	camera.fov = 25
	add_child(camera)
	
static func _sample2(arr: Array, t: float) -> Vector3:
	# This can't be right, bg_55:11 goes straight through a pillar
	# And yet it seems to work quite well for title.prd
	t = clamp(t, 0.0, 1.0)
	var points = arr.duplicate()
	for n in range(arr.size() - 1, 0, -1):
		for i in n:
			points[i] = points[i].lerp(points[i + 1], t)
	return points[0]
	
static func _sample(arr: Array, t: float) -> Vector3:
	# Very strong argument for this method from bg_04:o03 : it goes under the log where _sample2 goes through it
	# Nevermind, in-game it clips through the log: Screencast_20260227_235444.webm
	var m = arr.size() - 1
	t = max(t, 0.0)
	var i = min(int(floor(t)), m)
	var f = min(1.0, t - i)
	
	var p0 = arr[i]
	var p1 = arr[min(i + 1, m)]
	return lerp(p0, p1, f)
	
func _process(delta: float) -> void:
	anim_t += delta
	if true:
		var numSegments = max(1, pos_s.size() - 1)
		camera.position = _sample2(pos_s, anim_t / duration_seconds)
		camera.look_at(_sample2(focus_s, anim_t / duration_seconds))
	else:
		camera.position = pos_curve.sample_baked(anim_t / duration_seconds * pos_curve.get_baked_length())
		camera.look_at(focus_curve.sample_baked(anim_t / duration_seconds * focus_curve.get_baked_length()))
	
func _play_camera():
	process_mode = Node.PROCESS_MODE_INHERIT
	anim_t = 0.0
	camera.make_current()
