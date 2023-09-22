local depB = task(function()
    print("depB")
end)

local depC = task(function()
    print("depC")
end)

local depA = task(depC, function()
    print("depA")
end)

Entry = task({ depA, depB }, function()
    print("entrypoint")
end)
