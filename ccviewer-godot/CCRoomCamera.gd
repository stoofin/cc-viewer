extends Camera3D

func _ready():
	projection = Camera3D.PROJECTION_FRUSTUM
	size = 0.1
	
func _shift_layers(amount: Vector2) -> void:
	for child in get_children():
		if child is MeshInstance3D:
			child.position.x += amount.x * child.position.z
			child.position.y += amount.y * child.position.z
func _scale_layers(amount: float) -> void:
	for child in get_children():
		if child is MeshInstance3D:
			child.scale *= amount
	
func _unhandled_input(event: InputEvent) -> void:
	if get_viewport().get_camera_3d() != self: return
	if event is InputEventMouseMotion:
		if event.button_mask & MouseButtonMask.MOUSE_BUTTON_MASK_RIGHT:
			frustum_offset.x -= event.screen_relative.x * 2.0 * size / get_viewport().get_visible_rect().size.x
			frustum_offset.y += event.screen_relative.y * 2.0 * size / get_viewport().get_visible_rect().size.x
		#elif event.button_mask & MouseButtonMask.MOUSE_BUTTON_MASK_RIGHT and event.get_modifiers_mask() & KEY_MASK_CTRL:
			#_scale_layers(pow(2.0, event.screen_relative.y / 1000.0))
		#elif event.button_mask & MouseButtonMask.MOUSE_BUTTON_MASK_RIGHT:
			#_shift_layers(event.screen_relative * 2.0 * size / get_viewport().get_visible_rect().size.x * Vector2(-1.0, 1.0))
	elif event is InputEventMouseButton:
		if event.button_index == MouseButton.MOUSE_BUTTON_WHEEL_UP:
			size *= pow(2.0, -0.25)
		elif event.button_index == MouseButton.MOUSE_BUTTON_WHEEL_DOWN:
			size *= pow(2.0, 0.25)
		elif event.button_index == MouseButton.MOUSE_BUTTON_LEFT:
			get_viewport().gui_release_focus()
