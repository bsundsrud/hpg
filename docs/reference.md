# HPG Reference

## Tasks

Tasks are the primary execution unit in HPG. They define actions
to run (Lua code), along with a name for the task and a list of
any other tasks that this one depends upon.

Under the hood, the tasks are arranged into a DAG according to
their dependency trees, and then executed in topological order.

See [task()](#task).

## Targets

Targets are task names. You use them on the command line to request
task execution, or you can declare default targets that will be run
when the `-D` flag is set.

Default targets can be used, for example, to choose between different
task trees at runtime on different machines, so configuration for
multiple machines can live in one source code repository.

See [target()](#target).

## Execution Model

HPG executes in two passes: Definition and Execution. During Definition,
Tasks and Targets are discovered. If `-D` is specified, any Targets will
be added to the list of tasks to execute.

Tasks and Targets are only discovered at Definition time, Tasks or
Targets defined inside Tasks or unexecuted code won't be available.

At the end of the Definition phase, the DAG is created and sorted and
handed to the Execution phase, which runs the tasks in dependent order.

During Execution, any task that returns `cancel()` will cause any downstream
task to be skipped, but not otherwise interfere with execution. Any task that
results in a runtime error (or explicitly calls `fail()`) will immediately
terminate execution.

## Code Organization

The root config file is named `hpg.lua` by default (can be overridden
with the `-c` option). As configs grow, splitting tasks and other code
out into other files keeps things sane.

Lua packages work well for this, see [Programming in Lua §15](https://www.lua.org/pil/15.html).

## API Reference

Also see the [Lua Reference Manual](https://www.lua.org/manual/5.3/manual.html).
The following Lua Standard Library modules are included in HPG:
* Basic
* Table
* String
* utf-8
* Math
* Package

### Tasks and Targets

<a id="task"></a>
`task(name, dependencies, f)` - Define a task with dependencies and a
function body.
* `name`: String - defining the task name. Task names can be used as Targets.
* `dependencies`: String sequence - List of tasks that this task depends upon.
* `f`: Function() - Optional function body executed when task is executed. Receives
  no arguments, and return type is `nil` or a result, such as
  [`success()`](#success) or [`cancel()`](#cancel). If omitted, function is assumed
  to be successful.
* Example:
  ```lua
  task("other", {"root"}, function()
    echo("other")
  end)

  task("root", {}, function()
    echo("hello")
  end)

  --[[
  Output of `hpg other`:
  task [ root ]:
    echo:
      "hello"
  task [ other ]:
    echo:
      "other"
  ]]
  ```

<hr>

<a id="target"></a>
`target(target...)` - Declare one or more tasks as default targets.
* `target`: String - a variadic list of task names.

<hr>

<a id="success"></a>
`success()` - Sigil to be used as a return value from a task.
* Example:
  ```
  task("my-task", {}, function()
    return success()
  end)
  ```

<hr>

<a id="cancel"></a>
`cancel(reason)` - Return this from a task to cancel the current task and
skip any downstream task.
* `reason`: optional String - reason for cancellation.
* Example:
  ```
  task("my-task", {}, function()
    return cancel()
  end)
  ```

<hr>

<a id="fail"></a>
`fail(reason)` - Call to immediately halt execution and exit with an error.
* `reason`: String - reason for failure.

### Functions

Helper functions to make script-writing easier.

<hr>

<a id="echo"></a>
`echo(value)` - Pretty-prints the Lua value to stdout.
* `value`: Any - Any Lua value.

<hr>

<a id="file_contents"></a>
`file_contents(path)` - Return the contents of the given file as a String.
* `path`: String - Path to the file to read.
* Returns String - the file contents.

<hr>

<a id="file_exists"></a>
`file_exists(path)` - Return whether or not the given file exists.
* `path`: String - Path to the file to check for existence.
* Returns bool - true if the file exists, false otherwise.

<hr>

<a id="file_hash"></a>
`file_hash(path)` - Compute the Sha-256 hash of a file.
* `path`: String - Path to the file to hash.
* Returns String - the hex string of the full Sha-256 hash.

<hr>

<a id="from_json"></a>
`from_json(s)` - Parse the given JSON string as a Lua value.
* `s`: String - JSON body.
* Returns Value - A Lua Value representing the JSON body.

<hr>

<a id="group_exists"></a>
`group_exists(groupname)` - Check if a group already exists on a system.
* `groupname`: String - Name of group.
* Returns `bool`: `true` if group exists, `false` otherwise.

<hr>

<a id="hash"></a>
`hash(value)` - Compute the Sha-256 hash of a string.
* `value`: String - String to hash.
* Returns String - the hex string of the full Sha-256 hash.

<hr>

<a id="user_exists"></a>
`user_exists(username)` - Check if a user already exists on a system.
* `username`: String - Username of user.
* Returns `bool`: `true` if user exists, `false` otherwise.


### Actions

These functions perform some action that may have side effects on the system.

<hr>

<a id="append"></a>
`append(path, options)` - Append to an existing file, without overwriting
unrelated sections. Appended region will be delimited by marker lines on either
side, along with the Sha-256 hash of the region.  Will not update the file if
region is unchanged.
* `path`: String - Path to destination file.
* `options`: Table
  * `src`: String - Source file to read from. Exclusive with `contents`.
  * `contents`: String - Text to append.  Exclusive with `src`.
  * `marker`: String - Text to insert as a region marker.  Use unique comments,
    according to the file type.
  * `template`: optional bool (default `false`) - Whether or not to consider
    `src` or `contents` as a Tera template.
  * `context`: optional Table (default `{}`) - Variables available within a template.

<hr>

<a id="chmod"></a>
`chmod(file, mode)` - Change access modifiers for the given file.
* `file`: String - path to file to be changed.
* `mode`: String - Octal file mode (such as `"0644"` or `"0755"`)

<hr>

<a id="chown"></a>
`chown(file, opts)` - Change file user/group ownership.
* `file`: String - path to file to be changed.
* `opts`: Table
  * `user`: optional String - User to assign file to.
  * `group`: optional String - Group to assign file to.

<hr>

<a id="copy"></a>
`copy(src, dst, opts)` - Copy files, executing Tera templates if needed.  Does
not change the destination file if source and destination hashes match.
* `src`: String - Source file to copy, relative to current directory.
* `dst`: String - Destination file, relative to current directory.
* `opts`: optional Table
  * `template`: optional bool (default `false`) - Whether or not to consider
    `src` or `contents` as a Tera template.
  * `context`: optional Table - Variables available within a template.
  * `mode`: optional String - Octal file mode (such as `"0644"` or `"0755"`).
  * `user`: optional String - User to assign file to.
  * `group`: optional String - Group to assign file to.

<hr>

<a id="exec"></a>
`exec(cmd, opts)` - Run an executable as a subprocess.
* `cmd`: String - Path to executable to run.
* `opts`: optional Table
  * `args`: optional Table - List of arguments to pass to the command.
  * `inherit_env`: optional bool (default `true`) - Whether this command
    inherits environment variables from this process.
  * `env`: optional Table - Map of key/value pairs to insert into the
    environment of the command.
  * `cwd`: optional String - Working directory of command.
  * `stdout`: optional bool (default `true`) - Capture stdout of the process.
  * `stderr`: optional bool (default `true`) - Capture stderr of the process.
  * `echo`: optional bool (default `true`) - Echo stdout and stderr to HPG's stdout.
  * `ignore_exit`: optional bool (default `false`) - If `true`, will not throw
    an error on nonzero exit status.
* Returns a Table:
  * `status`: Number - Numeric exit status.
  * `stdout`: String - Output sent to stdout.
  * `stderr`: String - Output sent to stderr.

<hr>

<a id="group"></a>
`group(name, opts)` - Create or modify a group.
* `name`: String - Name of group to create/modify.
* `opts`: optional Table
  * `gid`: optional Number - directly assign a gid
  * `is_system`: optional bool (default `false`) - Whether group is a system group.

<hr>

<a id="mkdir"></a>
`mkdir(path, opts)` - Create a directory and any parent directories, as need.
* `path`: String - Directory path to create.
* `opts`: optional Table
  * `mode`: optional String - Octal directory mode.
  * `user`: optional String - User to assign directory to.
  * `group`: optional String - Group to assign directory to.

<hr>

<a id="packaging"></a>
`packaging(mgr, opts)` - Perform package management operations.
* `mgr`: String - Package manager to use. Currently supports `apt` and `pacman`.
* `opts`: Table
  * `update`: optional bool (default `false`) - Refresh package lists from distro.
  * `install`: optional Table - List of packages to install. Either a list of
    strings like `{ "package1", "package2" }` or a list of Tables with versions,
    like `{ { name = "package1", version = "1" }, { name = "package2", version = "1.1" } }`.
  * `remove`: optional Table - List of package names to remove.
* Returns a Table:
  * `installed`: Table - List of Tables with installed version info: fields
    `name`, `status`, and `version`. `status` is one of (`"installed"`,
    `"notfound"`, `"notinstalled"`, `"requested"`).
  * `removed`: Table - List of packages removed
  * `updated`: bool - whether or not a repo update was performed

<hr>

<a id="shell"></a>
`shell(cmd, opts)` - Run a command via a subshell. Copies `cmd` to a text file
and executes that file as a shell script.
* `cmd`: String - Shell command to run.
* `opts`: optional Table
  * `inherit_env`: optional bool (default `true`) - Whether this command
    inherits environment variables from this process.
  * `env`: optional Table - Map of key/value pairs to insert into the
    environment of the command.
  * `cwd`: optional String - Working directory of command.
  * `stdout`: optional bool (default `true`) - Capture stdout of the process.
  * `stderr`: optional bool (default `true`) - Capture stderr of the process.
  * `echo`: optional bool (default `true`) - Echo stdout and stderr to HPG's stdout.
  * `ignore_exit`: optional bool (default `false`) - If `true`, will not throw
    an error on nonzero exit status.
  * `sh`: optional String (default `/bin/sh`) - Shell to run this command with.
  * `sh_args`: optional Table (default `{}`) - Extra arguments to pass to the shell.
* Returns a Table:
  * `status`: Number - Numeric exit status.
  * `stdout`: String - Output sent to stdout.
  * `stderr`: String - Output sent to stderr.

<hr>

<a id="symlink"></a>
`symlink(src, dst, opts)` - Create a symlink.
* `src`: String - Source path of the symlink.
* `dst`: String - Destination path of the symlink.
* `opts`: optional Table
  * `mode`: optional String - Octal file mode.
  * `user`: optional String - User to assign symlink to.
  * `group`: optional String - Group to assign symlink to.

<hr>

<a id="touch"></a>
`touch(path, opts)` - Create an empty file, if it does not already exist.
* `path`: String - Path to file, relative to current directory.
* `opts`: optional Table
  * `mode`: optional String - Octal file mode.
  * `user`: optional String - User to assign symlink to.
  * `group`: optional String - Group to assign symlink to.

<hr>

<a id="user"></a>
`user(name, opts)` - Create or modify a user.
* `name`: String - Username of user to create/modify.
* `opts`: Table
  * `comment`: optional String - Comment for user, also used as "Real Name".
  * `home_dir`: optional String - Home directory of user.
  * `group`: optional String - Primary group of user.
  * `groups`: optional Table - String list of supplemental groups to add user to.
  * `is_system`: optional bool (default `false`) - Is user a system account?
  * `create_home`: optional bool (default `false`) - Create the user's home
    directory, if it doesn't exist?
  * `create_user_group`: optional bool (default `true`) - Create a group with
    the same name as the user, for use as the primary group?
  * `uid`: optional Number - Directly set uid of user, rather than be assigned one.
  * `shell`: optional String (default `"/usr/bin/nologin"`) Login shell for user.
