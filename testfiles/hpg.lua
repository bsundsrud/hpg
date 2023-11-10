foo = task("foo task", {}, function()
  print "from foo"
  echo "echoing from foo"
end)

bar = task("bar task", { foo }, function()
  print "from bar"
end)

baz = task("baz task", foo, function()
  print "from baz"
end)

quux = task("quux", { bar, baz }, function()
  print "from quux"
end)

roles = task("roles task", function()
  local meta = from_json(file_contents("meta.json"))
  echo(meta)
end)

empty = task("empty task", roles)

test_files = task("test-files task", function()
  local f = file("meta.json")
  local meta = from_json(f:contents())
  echo(meta)
  file("doesnt-exist")
      :touch()
      :chmod("0755")
      :copy("copied-file")
end)

apt_update = task("apt-update", function()
  pkg.apt.update()
  pkg.apt.update()
  local s = pkg.apt.status("firefox")
  echo(s)
end)

other = task("other task", function()
  echo("sure")
end)

machine_info = task("machine-info task", function()
  echo(machine)
end)



rootA = task("rootA", function()
  echo("hello")
end)

rootB = task("rootB", function()
  echo("world")
end)

middle = task("middle", { rootA, rootB })
tree = task("tree dependencies", middle, function()
  echo("other")
end)


file_path = task("Try file path", function()
  local d = dir("/tmp")
  echo(d.path)
  echo(d.canonical_path)
  local f = file(d.path .. "/foo.txt")
  echo(f.path)
end)
