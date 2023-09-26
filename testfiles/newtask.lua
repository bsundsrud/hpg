local depB = task("dep B", function()
    echo("depB")
end)

local depC = task("dep C", function()
    echo("depC")
end)

local depA = task("dep A", depC, function()
    echo("depA")
end)

Entry = task("main task", { depA, depB }, function()
    echo("entrypoint")
end)

target(Entry, depA)

shell_cmd = task("Run a shell command", Entry, function()
    shell("ls")
end)

ls_cmd = task("Run ls -l", shell_cmd, function()
    shell("ls -l")
end)
