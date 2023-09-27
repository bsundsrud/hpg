test_homedir = task("test_homedir", function()
    local v = homedir()
    echo(v)
    local v2 = homedir("root")
    echo(v2)

end)
