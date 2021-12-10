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