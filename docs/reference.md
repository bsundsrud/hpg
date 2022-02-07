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
`task(name, dependencies, f)`
`task(name, f)`
`task(name, dependencies)` - Define a task with dependencies and a function body.
* `name`: String - defining the task name. Task names can be used as Targets.
* `dependencies`: Optional String or String Sequence - List of tasks that this task depends upon.
* `f`: Function() - Optional function body executed when task is executed. Receives
  no arguments, and return type is `nil` or a result, such as
  [`success()`](#success) or [`cancel()`](#cancel). If omitted, function is assumed
  to be successful.
* Example:
  ```lua
  task("other", "middle", function()
    echo("other")
  end)

  task("middle", {"rootA", "rootB"})

  task("rootA", function()
    echo("hello")
  end)

  task("rootB", function()
    echo("world")
  end)

  --[[
  Output of `hpg other`:
  task [ rootA ]:
    echo:
      "hello"
  task [ rootB ]:
    echo:
      "world"
  task [ middle ]:
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

### Classes

#### `Dir` Class

Class for handling directories.

##### Constructor

`dir(path)` - Return a new `Dir` object. `path` must be a directory, otherwise
    a runtime error will be raised.

##### `Dir` Methods

<a id="dir-chmod"></a>
`chmod(mode)` - Change access modifiers for the directory.
* `mode`: String - Octal file mode (such as `"0644"` or `"0755"`)
* Returns the `Dir` object for chaining.

<a id="dir-chown"></a>
`chown(opts)` - Change directory user/group ownership.
* `opts`: Table
  * `user`: optional String - User to assign file to.
  * `group`: optional String - Group to assign file to.
* Returns the `Dir` object for chaining.
<a id="mkdir"></a>

<a id="dir-mkdir"></a>
`mkdir()` - Create a directory and any parent directories, as needed.
* Returns the `Dir` object for chaining.

<a id="dir-symlink"></a>
`symlink(src, dst, opts)` - Create a symlink from this directory to the destination.
* `dst`: String - Destination path of the symlink.
* Returns the `Dir` object of the new directory.

<hr>

#### `File` Class

Class for handling files and their contents.

##### Constructor

`file(path)` - Returns new `File` object.  `path` must be a file, otherwise a
    runtime error will be raised.

##### `File` Methods

<a id="file-append"></a>
`append(destination, options)` - Append to an existing file, without overwriting
unrelated sections. Appended region will be delimited by marker lines on either
side, along with the Sha-256 hash of the region.  Will not update the file if
region is unchanged.
* `destination`: String - Path to destination file.
* `options`: Table
  * `src`: String - Source file to read from. Exclusive with `contents`.
  * `contents`: String - Text to append.  Exclusive with `src`.
  * `marker`: String - Text to insert as a region marker.  Use unique comments,
    according to the file type.
* Returns a table:
  * `updated`: boolean, whether or not the section was appended or changed.

<a id="file-append_template"></a>
`append_template(destination, options)` - Append to an existing file, without overwriting
    unrelated sections. Evaluates the source as a template. Appended region
    will be delimited by marker lines on either side, along with the Sha-256 hash
    of the region.  Will not update the file if region is unchanged.
* `destination`: String - Path to destination file.
* `options`: Table
  * `src`: String - Source file to read from. Exclusive with `contents`.
  * `contents`: String - Text to append.  Exclusive with `src`.
  * `marker`: String - Text to insert as a region marker.  Use unique comments,
    according to the file type.
  * `context`: optional Table (default `{}`) - Variables available within a template.
* Returns a table:
  * `updated`: boolean, whether or not the section was appended or changed.

<a id="file-chmod"></a>
`chmod(mode)` - Change access modifiers for the file.
* `mode`: String - Octal file mode (such as `"0644"` or `"0755"`)
* Returns the `File` object for chaining.

<a id="file-chown"></a>
`chown(opts)` - Change file user/group ownership.
* `opts`: Table
  * `user`: optional String - User to assign file to.
  * `group`: optional String - Group to assign file to.
* Returns the `File` object for chaining.

<a id="file-copy"></a>
`copy(dst)` - Copy file to destination.  Does not change the destination
    file if source and destination hashes match.
* `dst`: String - Destination file, relative to current directory.
* Returns a table:
  * `updated`: boolean, whether or not the file was changed.

<a id="file-contents"></a>
`contents()` - Return the contents of the given file as a String.
* Returns String - the file contents.

<a id="file-exists"></a>
`exists()` - Return whether or not the given file exists.
* Returns bool - true if the file exists, false otherwise.

<a id="file-hash"></a>
`hash()` - Compute the Sha-256 hash of a file.
* Returns String - the hex string of the full Sha-256 hash.

<a id="file-symlink"></a>
`symlink(dst)` - Create a symlink.
* `dst`: String - Destination path of the symlink.
* Returns the `File` object for the destination.

<a id="file-template"></a>
`template(dst, context)` - Copy file to destination, evaluating it as a template first.
    Does not change the destination file if source and destination hashes match.
* `dst`: String - Destination file, relative to current directory.
* `context`: optional Table - Variables available within a template.
* Returns a table:
  * `updated`: boolean, whether or not the file was changed.

<a id="file-touch"></a>
`touch(path)` - Create an empty file, if it does not already exist.
* Returns the `File` object for chaining.

### Modules

#### `pkg` Module - System Package Manager integration

##### `apt` Submodule - Perform apt-get actions

<a id="pkg-apt-update"></a>
`update(force)` - Update repo list.  Will only update repos once per
    HPG run unless `force` is `true`.
* `force` - Optional boolean to force repo update even if it has already
  happened this run.  Default `false`.
* Returns boolean - `true` if repos were updated, `false` if not.

<a id="pkg-apt-install"></a>
`install(packages)` - Install packages with `apt-get`.
* `packages` - List of packages to install. Either a list of
    strings like `{ "package1", "package2" }` or a list of Tables with versions,
    like `{ { name = "package1", version = "1" }, { name = "package2", version = "1.1" } }`.

<a id="pkg-apt-status"></a>
`status(package)` - Get current status of given package.
* `package` - name of package to query for.
* Returns a Table with members `name`, `status`, and `version`.  `status` will be one of
  `"installed"`, `"notfound"`, `"requested"`, or `"notinstalled"`.

<a id="pkg-apt-remove"></a>
`remove(packages)` - Remove packages from the system.
* `packages` - List of package names to remove.
* Returns a List of Tables with members `name`, `status`, and `version`, one for each
  package that was requested to be removed.  `status` will be one of `"installed"`,
  `"notfound"`, `"requested"`, or `"notinstalled"`.

### Functions

Helper functions to make script-writing easier, are side-effect free.

<hr>

<a id="echo"></a>
`echo(value)` - Pretty-prints the Lua value to stdout.
* `value`: Any - Any Lua value.

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
