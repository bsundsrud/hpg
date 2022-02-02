task("foo", {}, function ()
        print "from foo"
        echo "echoing from foo"
end)

task("bar", {"foo"}, function()
  print "from bar"
end)

task("baz", {"foo"}, function()
  print "from baz"
end)

task("quux", {"bar", "baz"}, function()
  print "from quux"
end)

task("roles", {}, function()
  local meta = from_json(file_contents("meta.json"))
  echo(meta)
end)

task("empty", {"roles"})

task("test-files", {}, function()
  local f = file("meta.json")
  local meta = from_json(f:contents())
  echo(meta)
  file("doesnt-exist")
     :touch()
     :chmod("0755")
     :copy("copied-file")
end)

task("apt-update", {}, function()
        pkg.apt.update()
        pkg.apt.update()
        local s = pkg.apt.status("firefox")
        echo(s)
end)

task("machine-info", {}, function()
        echo(machine)
end)
