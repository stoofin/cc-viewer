class_name LoadTimer

static var stack := []

static func start(name: String):
	stack.push_back([name, Time.get_ticks_usec()])
static func stop():
	var entry = stack.pop_back()
	var end = Time.get_ticks_usec()
	print("%s took %.2fms" % [entry[0], (end - entry[1]) / 1000.0])
