---@meta hpg
---@diagnostic disable: lowercase-global, missing-return

--- Task sigil type
---@class Task
local Task = {}
--- Defines an HPG task. Task bodies are not evaluated until after task dependencies and execution order has been defined.
---@param description string Human description of the task
---@param dependency? Task|Task[] Other task names that must run before this one.
---@param body? function Task body, code to run on task execution.
---@return Task Task object that can be used for dependencies
---@overload fun(description: string, body: function)
function task(description, dependency, body)
end

--- Define tasks that should be run as default (via `hpg -D`).
---@param task string|Task Task name to register as a default.
---@vararg string
function target(task, ...)
end

--- Creates a sigil that marks task success.
--- use `return success()` in a task to immediately succeed the task.
---@return userdata sigil success marker
function success()
end

--- Create a sigil for task cancellation (with optional reason).
--- Use `return cancel(reason)` in a task to immediately cancel the
--- current task and skip any downstream dependent task.
---@param reason? string Reason for cancellation.
---@return userdata sigil cancellation marker
function cancel(reason)
end

--- Create a sigil for task failure.
--- Use `return fail(reason)` in a task to immediately fail the task
--- and stop all task execution.
---@param reason string Reason for failure
---@return userdata sigil failure marker
function fail(reason)
end

--- Hpg variables that were passed in via file or command line.
--- Values assigned inside a lua file will be considered defaults, and will be used with less precedence than passed-in variables.
---@type table
vars = {}

--- Create a new instance of a Dir object.
--- Runtime error raised if path exists and is not a directory.
---@param path string Directory path in unix format, relative to current working directory.
---@return Dir dir Dir instance
function dir(path)
end

--- Create a Dir object pointing to a user's home directory.
---- Runtime error raised if user's home directory is not available.
---@param user? string Optional user to get the home directory for.  If omitted, current effective user is assumed.
---@return Dir dir Dir instance
function homedir(user)
end

--- Represents a filesystem directory.
---@class Dir
---@field path string Directory path, as constructed
---@field canonical_path string Canonical (fully resolved) directory path. Path must exist.
local Dir

--- Change directory modes.
---@param mode string Octal file mode (such as "0700" or "0755").
---@return Dir dir Original `dir` instance.
function Dir:chmod(mode)
end

--- Options table for chown.
---@class ChownOpts
---@field user string? User to assign object to.
---@field group string? Group to assign object to.

--- Options table for chown directory.
---@class DirChownOpts
---@field user string? User to assign object to.
---@field group string? Group to assign object to.
---@field recursive boolean? Whether or not to apply chown recursively. Default `false`.


--- Change ownership of a directory.
---@param opts DirChownOpts
---@return Dir dir Original `Dir` instance.
function Dir:chown(opts)
end

--- Test for directory existence.
---@return boolean exists `true` if the directory exists, `false` otherwise.
function Dir:exists()
end

--- Create directory and any parent directories, as needed.
---@return Dir dir `Dir` instance for this path.
function Dir:mkdir()
end

--- Create a symlink from this directory to the destination.
---@param dst string Destination path of the symlink, relative to current working directory.
---@return Dir dst `Dir` object for the destination.
function Dir:symlink(dst)
end

--- Copy directory to destination.
---@param dst string Destination path of the directory, relative to current working directory.
---@return Dir dst `Dir` object for the destination.
function Dir:copy(dst)
end

--- Copy all directory contents to destination.
---@param dst string Destination path of the directory, relative to current working directory.
---@return Dir dst `Dir` object for the destination.
function Dir:copy_contents(dst)
end

--- Create a new instance of the `File` class.
--- Raises a runtime error if the path exists and is not a file.
---@param path string Path to file.
---@return File file `File` instance.
function file(path)
end

--- Represents a file on the filesystem.
---@class File
---@field path string File path, as constructed
---@field canonical_path string Canonical (fully resolved) file path. Path must exist.
local File = {}

---@class FileAppendOpts
---@field src? string Source file to read from. Exclusive with `contents`.
---@field contents? string Text to append. Exclusive with `src`.
---@field marker string Text to insert as a region marker. Should be a comment line, according to file type.

--- Append to an existing file, without overwriting unrelated sections.
--- Appended region will be delimited by marker lines on either side, along with the SHA-256 hash of the region.
--- Will not update the file if the region is unchanged.
---@param options FileAppendOpts Table of append options.
---@return boolean changed Whether or not the section was appended or updated.
function File:append(options)
end

---@class FileAppendTemplateOpts
---@field src? string Source file to read from. Exclusive with `contents`.
---@field contents? string Text to append. Exclusive with `src`.
---@field marker string Text to insert as a region marker. Should be a comment line, according to file type.
---@field context table<string, any>? Variables available within the template.

--- Append to an existing file, without overwriting unrelated sections.
--- Evaluates `options.src` or `options.contents` as a Tera template.
--- Appended region will be delimited by marker lines on either side, along with the SHA-256 hash of the region.
--- Will not update the file if the region is unchanged.
---@param options FileAppendTemplateOpts Table of append options.
---@return boolean changed Whether or not the section was appended or updated.
function File:append_template(options)
end

--- Change file modes.
---@param mode string Octal file mode (such as "0644" or "0755").
---@return File file Original `file` instance.
function File:chmod(mode)
end

--- Change ownership of a file.
---@param opts ChownOpts
---@return File file Original `File` instance.
function File:chown(opts)
end

--- Return the contents of a given file as a string.
---@return string contents The file contents.
function File:contents()
end

--- Copy file to destination.
--- Does not change the destination file if source and destination hashes match.
---@param dst string Destination file path, relative to current working directory.
---@return boolean changed Whether or not the file was updated.
function File:copy(dst)
end

--- Test for file existence.
---@return boolean exists `true` if the file exists, `false` otherwise.
function File:exists()
end

--- Compute the SHA-256 hash of the file.
---@return string hash The hex string of the full SHA-256 hash.
function File:hash()
end

--- Create a symlink from this file to the destination.
---@param dst string Destination path of the symlink, relative to current working directory.
---@return Dir dst `File` object for the destination.
function File:symlink(dst)
end

--- Copy file to destination, evaluating it as a template first.
--- Does not change the destination file if source and destination hashes match.
---@param dst string Destination file, relative to current directory.
---@param context table<string, any>? Variables available within a template.
---@return boolean changed Whether or not the file was updated.
function File:template(dst, context)
end

--- Create an empty file, if it does not already exist.
---@return File file The original `File` object.
function File:touch()
end

--- System-wide package management interface.
---@class pkg
---@field apt Apt
pkg = {}

--- Apt-get packaging for Debian derivatives.
---@class Apt
pkg.apt = {}

--- Update repo list.
--- Will only update repos once per HPG run unless `force` is `true`.
---@param force boolean? Force a repo update.
---@return boolean updated Whether or not the repos were updated.
function pkg.apt.update(force)
end

--- (Name, Version) pair for package install requests.
---@class PackageInstall
---@field name string Package name.
---@field version string Package version.

---@alias PackageInstallationStatus
---| "installed" # Package is currently installed.
---| "notfound" # Package was not found in the repos.
---| "requested" # Package was requested to install, but has not completed installation.
---| "notinstalled" # Package is not installed.

--- Current status of a package.
---@class PackageStatus
---@field name string Package name.
---@field version string Package version.
---@field status PackageInstallationStatus Package installation status.

--- Install packages with `apt-get`.
---@param packages string[]|PackageInstall[] Packages to install.
---@return PackageStatus status Status of requested packages.
function pkg.apt.install(packages)
end

--- Current status of package.
---@param package string Package name.
---@return PackageStatus status Status of package.
function pkg.apt.status(package)
end

--- Remove packages from the system.
---@param packages string[] List of packages to remove.
---@return PackageStatus[] statuses List of statuses of removed packages.
function pkg.apt.remove(packages)
end

--- Output of Ensure command
---@class EnsureTable
---@field updated boolean False if all packages were at requested versions, True if package installation was attempted/requested
---@field packages PackageStatus[] List of packages installed/requested

--- Ensure all given packages are installed on a system.  If any are missing, it will call `pkg.apt.update(false)` and `pkg.apt.install(packages)`.
---@param packages string[]|PackageInstall[] Packages to install.
---@return EnsureTable status Table with `updated` and `packages`. `updated` is true if installation was attempted, `packages` contains the packages sent to apt.
function pkg.apt.ensure(packages)
end

--- Pretty-prints the Lua value to stdout.
---@param value any Value to print.
function echo(value)
end

--- Parse the given JSON string as a Lua value.
---@param s string JSON body to parse.
---@return any # A Lua Value representing the JSON body.
function from_json(s)
end

--- Check if a group already exists on a system.
---@param groupname string Name of group.
---@return boolean exists `true` if group exists, `false` otherwise.
function group_exists(groupname)
end

--- Compute the SHA-256 hash of a string.
---@param value string String to hash.
---@return string hash SHA-256 hash as hex string.
function hash(value)
end

--- Check if a user already exists on a system.
---@param username string Username of user.
---@return boolean exists `true` if user exists, `false` otherwise.
function user_exists(username)
end

---@class ExecOpts
---@field args? string[] Arguments to pass to the command.
---@field inherit_env? boolean Inherit environment from this process. Default `true`.
---@field env? table<string, string> Environment variables to inject into subprocess.
---@field cwd? string Working directory of command.
---@field stdout? boolean Capture stdout of the process. Default `true`.
---@field stderr? boolean Capture stderr of the process. Default `true`.
---@field echo? boolean Echo stdout and stderr of process to HPG's stdout. Default `true`.
---@field ignore_exit? boolean If `true`, will not halt task execution on nonzero exit status.

---@class ExitStatus
---@field status number Numeric exit status of process.
---@field stdout string Stdout output of process.
---@field stderr string Stderr output of process.

--- Run an executable as a subprocess.
---@param cmd string Path to executable.
---@param opts? ExecOpts Options for subprocess.
---@return ExitStatus status Exit status and output from process.
function exec(cmd, opts)
end

---@class GroupOpts
---@field gid number? Directly assign a GID.
---@field is_system boolean? Whether group is a system group.

--- Create or modify a Unix group.
---@param name string Name of group to create/modify.
---@param opts GroupOpts? Options for group creation/modification.
function groupmod(name, opts)
end

---@class ShellOpts
---@field inherit_env? boolean Inherit environment from this process. Default `true`.
---@field env? table<string, string> Environment variables to inject into subprocess.
---@field cwd? string Working directory of command.
---@field stdout? boolean Capture stdout of the process. Default `true`.
---@field stderr? boolean Capture stderr of the process. Default `true`.
---@field echo? boolean Echo stdout and stderr of process to HPG's stdout. Default `true`.
---@field ignore_exit? boolean If `true`, will not halt task execution on nonzero exit status.
---@field sh? string Shell to run this command with. Default `/bin/sh`.
---@field sh_args? string[] Extra arguments to pass to the shell.

--- Run a command via a subshell.
--- Copies `cmd` to a text file and executes that file as a shell script.
---@param cmd string Shell command to run.
---@param opts ShellOpts? Options for shell command.
---@return ExitStatus status Exit status of shell command.
function shell(cmd, opts)
end

---@class UserOpts
---@field comment? string Comment for user, also used as "Real Name".
---@field home_dir? string Home directory of user.
---@field group? string Primary group of user.
---@field groups? string[] String list of supplemental groups to add user to.
---@field is_system? boolean Set user account as a system account. Defaults to `false`.
---@field create_home? boolean Create the user's home directory, if it doesn't exist. Defaults to `false`.
---@field create_user_group? boolean Create a group with the same name as the user, for use as the primary group. Defaults to `true`.
---@field uid? number Directly set uid of user, rather than be assigned one.
---@field shell? string Login shell for user. Defaults to `/usr/bin/nologin/`.

--- Create or modify a user.
---@param name string Username of user to create/modify.
---@param opts UserOpts Options for user creation.
function usermod(name, opts)
end

---@class UserDef
---@field name string Username
---@field uid number User ID
---@field gid number Group ID
---@field gecos string GECOS field
---@field home_dir string User home directory
---@field shell string Login shell for user

--- Get information on an existing user.
---@param name string? Username to look up.  If omitted, defaults to current effective user
---@return UserDef userdata Table containing information from /etc/passwd
function user(name)
end

--- System information from uname(2)
---@class Uname
---@field sysname string Operating system name.
---@field nodename string Name of system.
---@field release string Kernel release.
---@field version string Kernel version.
---@field machine string Machine hardware name.

--- Information about the machine that HPG is running on.
machine = {}

---@type Uname
machine.uname = {}

---@alias ArchiveType
---| "zip" # ZIP file
---| "tar" # Tarball

---@alias CompressionType
---| "gz" # Gzip
---| "bz2" # Bzip2

---@class ArchiveOpts
---@field type ArchiveType? Type of the archive.  If not present, guessed from filename.
---@field compression CompressionType? Compression type of archive.  If not present, guessed from filename.

--- Create a new instance of `Archive`.
---@param path string Path to archive file.
---@param opts? ArchiveOpts Options for the archive.
---@return Archive archive `Archive` instance.
function archive(path, opts)
end

--- Represents a compressed archive.
---@class Archive
local Archive = {}

--- Extract an archive.
---@param dst string Path to extract to.
---@return Dir dst Dir object for destination.
function Archive:extract(dst)
end

---@class InstallOpts
---@field url? string URL to fetch from.  If omitted, `archive_path` is assumed to exist already locally.
---@field hash? string SHA-256 hash of the archive.  If omitted, the archive will always be extracted.
---@field install_dir? string If omitted, defaults to `extract_dir`. Useful if the archive contains one or more layers of directories, it is then used to point at the final application directory.  The hash of the archive will be written into this dir at `.hpg-hash`.

--- Fetch and extract an archive.
--- Only extracts if the archive is not present or hash differs.
---@param archive_path string Local path to archive file. If installing from the network, this is where the file will be downloaded to.
---@param extract_dir string Destination directory for the archive.
---@param opts InstallOpts Extra options for install.
function install(archive_path, extract_dir, opts)
end

--- Represents a network resource.
---@class Url
local Url = {}

--- Create a `Url` instance that points to a network resource.
---@param u string URL for resource.
---@return Url # `Url` instance.
function url(u)
end

---@class FetchUrlOpts
---@field headers table<string, string>? Key/value pairs which will be inserted into the request as headers.
---@field expected_response number Expected HTTP response code.  Default is 200.

--- Performs a GET request against the resource and returns the body as a string.
---@param opts FetchUrlOpts? Fetch options.
---@return string body The body of the response.
function Url:get(opts)
end

--- Performs a GET request against the resource, parses the response body as JSON,
--- and returns the Lua representation of the JSON.
---@param opts FetchUrlOpts? Fetch options.
---@return any json JSON body as Lua value.
function Url:json(opts)
end

--- Performs a GET request against the resource and saves the response body to disk.
---@param dst string Destination path.
---@param opts FetchUrlOpts? Fetch options.
---@return File file `File` instance for payload.
function Url:save(dst, opts)
end

--- Represents a Systemd unit.
---@class SystemdUnit
local SystemdUnit = {}

--- Result of a Systemd job.
---@class SystemdJobResult
local SystemdJobResult = {}

---@alias SystemdJobResultStr string
---| "done" # Job completed successfully.
---| "canceled" # Job was canceled by another process.
---| "timeout" # Job was canceled due to timeout.
---| "failed" # Job failed with an abnormal exit status.
---| "dependency" # A dependent job failed to start.
---| "skipped" # Job was skipped because it did not apply to unit's current state.

--- Whether the job was considered successful.
--- result == "done"
---@return boolean success
function SystemdJobResult:successful()
end

--- Whether the job was considered failed.
--- result != "done"
---@return boolean success
function SystemdJobResult:failed()
end

--- The job status of the action.
---@return SystemdJobResultStr result
function SystemdJobResult:result()
end

--- Access to Systemd services.
systemd = {}

--- Interact with System systemd (Init daemon)
---@param unit string Name of the unit to control.
---@return SystemdUnit unit `SystemdUnit` instance.
function systemd.system(unit)
end

--- Interact with Session systemd (User daemon)
---@param unit string Name of the unit to control.
---@return SystemdUnit unit `SystemdUnit` instance.
function systemd.session(unit)
end

--- Reloads the current (system or session) systemd daemon.
function SystemdUnit:daemon_reload()
end

--- Start the unit.
---@return SystemdJobResult result The outcome of the start job.
function SystemdUnit:start()
end

--- Start the unit, or fail execution.
function SystemdUnit:must_start()
end

--- Stop the unit.
---@return SystemdJobResult result The outcome of the stop job.
function SystemdUnit:stop()
end

--- Stop the unit, or fail execution.
function SystemdUnit:must_stop()
end

--- Reload the unit.
---@return SystemdJobResult result The outcome of the reload job.
function SystemdUnit:reload()
end

--- Reload the unit, or fail execution.
function SystemdUnit:must_reload()
end

--- Restart the unit.
---@return SystemdJobResult result The outcome of the restart job.
function SystemdUnit:restart()
end

--- Restart the unit, or fail execution.
function SystemdUnit:must_restart()
end

--- Reload or restart the unit.
--- Attempts reload unless the unit does not support reloading.
---@return SystemdJobResult result The outcome of the reload job.
function SystemdUnit:reload_or_restart()
end

--- Reload or restart the unit, or fail execution.
--- Attempts reload unless the unit does not support reloading.
function SystemdUnit:must_reload_or_restart()
end

--- Enable the unit.
function SystemdUnit:enable()
end

--- Force-enable the unit.
function SystemdUnit:force_enable()
end

--- Disable the unit.
function SystemdUnit:disable()
end

--- Force-disable the unit.
function SystemdUnit:force_disable()
end

--- Mask the unit.
function SystemdUnit:mask()
end

--- Force-mask the unit.
function SystemdUnit:force_mask()
end

--- Unmask the unit.
function SystemdUnit:unmask()
end
