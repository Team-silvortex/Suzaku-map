# Map

## Reading this repository today

This file indexes the original conceptual design notes, not the current Rust
module tree or a claim that each concept is a separately implemented subsystem.
The Linux-first input method has since grown shared engine, vocabulary,
prediction, native-host and companion-panel implementations.

Start with the [product and Linux entry points](../README.md), then the
[current implementation responsibilities and ownership](../docs/functional-network.md#当前-rust-模块与职责).
The [runtime route diagram](../docs/functional-network.mmd) is a logical projection;
the [functional tensor](../docs/functional-tensor.json) records scoped capability
evidence, not a complete call graph or proof of current release acceptance.
Public Rust entry points are listed in [lib.rs](lib.rs).

The historical [XR/tablet application of the map](06-IME-XR-Tablet.md) explains
the early design intent. Keep it distinct from current platform support and
verified Linux behavior. The five notes below remain useful design principles;
they do not authorize input actions or replace the implemented host/IPC contracts.

## Original conceptual layers

This set is organized by system question, not by topic.

---

## Layers

### 1. Signal

Question:
What is true, changing, or unsafe to trust?

Document:

- [Signal](./01-Signal.md)

Boundary:
Observe and preserve evidence.
Do not decide or enforce.

### 2. Control

Question:
What actions must be gated, confirmed, or separated?

Document:

- [Control](./02-Control.md)

Boundary:
Constrain execution and responsibility.
Do not diagnose or recover.

### 3. Resilience

Question:
How does the system remain usable under stress, failure, or scarcity?

Document:

- [Resilience](./03-Resilience.md)

Boundary:
Preserve continuity.
Do not authorize or coordinate.

### 4. Coordination

Question:
How do actors maintain cooperation when institutions are weak or absent?

Document:

- [Coordination](./04-Coordination.md)

Boundary:
Organize continuity across actors.
Do not gate critical action.

### 5. Expression

Question:
How is semantic intent turned into valid structured output?

Document:

- [Expression](./05-Expression.md)

Boundary:
Define the expression interface.
Do not govern, recover, or coordinate.

---

## Separation Test

1. If it improves visibility, it is `Signal`.
2. If it limits action, it is `Control`.
3. If it preserves operation under constraint, it is `Resilience`.
4. If it organizes multi-actor continuity, it is `Coordination`.
5. If it builds valid output from intent, it is `Expression`.

If one note fits multiple tests, split it.
