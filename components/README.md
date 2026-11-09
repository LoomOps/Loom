# Extensions: Components, Tools and Profiles

User-owned Components and Tools live in the configured host store under `/loom/components` (Tools use its `.tools` subtree).
They are not part of the Ops image. Register source revisions and reusable execution
profiles through MCP `components`; see [the component guide](../loom_use/components.md).
Platform releases preserve editable sources and immutable revisions used by Jobs.

The Dashboard's **Extensions** inventory groups Components and Tools and shows
exact Profile bindings. Components implement Job lifecycle integration; Tools
implement reusable research algorithms, including hybrid runners. Profiles bind
execution contracts and immutable revisions. This UI label does not rename the
six MCP groups or the `components` actions.
