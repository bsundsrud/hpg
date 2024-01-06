# HPG Reference

## Tasks

Tasks are the primary execution unit in HPG. They define actions
to run (Lua code), along with a name for the task and a list of
any other tasks that this one depends upon.

Under the hood, the tasks are arranged into a DAG according to
their dependency trees, and then executed in topological order.


## Targets

Targets are task names. You use them on the command line to request
task execution, or you can declare default targets that will be run
when the `-D` flag is set.

Default targets can be used, for example, to choose between different
task trees at runtime on different machines, so configuration for
multiple machines can live in one source code repository.


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

For HPG's API, see [The Lua definitions](../src/hpgdefs.lua)