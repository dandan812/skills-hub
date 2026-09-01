# Skills Hub Custom Runtime Hook Boundary

This directory contains an inert producer example. Skills Hub Custom does not
install, enable, configure, or invoke an Agent Hook.

The fixed Windows inbox is:

```text
%LOCALAPPDATA%\com.dandan812.skillshubcustom\runtime-hooks\skill-runtime-v1.jsonl
```

Open the Runtime page and use Refresh once to create the owned directory and
file. The example writer is disabled unless `-EnableExample` is supplied. It
accepts only the V1 scalar fields and never reads prompts, transcripts, tool
arguments, outputs, environment dumps, or arbitrary JSON.

`skill.loaded` may be emitted only when a producer can prove that a Skill's
instructions were injected for that session. `skill.called` never implies
loaded. These same-user events are reported local evidence, not native Agent
attestation.
