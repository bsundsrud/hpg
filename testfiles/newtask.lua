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