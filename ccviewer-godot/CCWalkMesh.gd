extends MeshInstance3D

var navRegion := NavigationRegion3D.new()
var isSetup := false

func _ready():
	var navMesh := NavigationMesh.new()
	navMesh.create_from_mesh(mesh)
	navRegion.navigation_mesh = navMesh
	add_child(navRegion)
	NavigationServer3D.map_changed.connect(func(_map): isSetup = true)
	var btn := CheckButton.new()
	btn.text = "WalkMesh"
	btn.button_pressed = true
	get_tree().root.find_child("TempControls", true, false).add_child(btn)
	btn.toggled.connect(func(b): visible = b)

	for el in get_meta("extras", {}).get("walkmesh-debug", []): match el:
		{"position": [var x, var y, var z], "info": var info}:
			#print(info)
			var label := Label3D.new()
			add_child(label)
			label.text = "%02X" % info
			label.position = Vector3(x, y, z)
		_:
			push_error("Unknown debug info")
	
func _exit_tree():
	isSetup = false

func closest_point(to_point: Vector3) -> Vector3:
	if isSetup:
		return NavigationServer3D.map_get_closest_point(get_viewport().world_3d.navigation_map, to_point)
	else:
		return to_point
