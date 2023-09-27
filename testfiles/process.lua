Test1 = task("Test 1", function()
    shell("ls -l; sleep 1; ls -l >&2; sleep 1; ls -l", {
        stdout = false,
    })
end)

Test2 = task("streaming dnf update", function()
    shell("dnf update -y", {
        stdout = false,
        stderr = true,
    })
end)