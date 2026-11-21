# Convergence fixtures

This is the canonical root for deterministic source-to-swarm convergence
fixture inputs. Fixtures use normalized repository identities, fixed Git
authors and timestamps where object identity matters, explicit line-ending and
file-mode expectations, and no live GitHub, network, user-home, or wall-clock
dependency.

This root is manifest-only until #3324 adds the first executable corpus. Its
current contract is owned by RIPR-SPEC-0167 and the convergence architecture
gate, so it must not fabricate an empty `diff.patch` or expected result merely
to resemble a runnable fixture.

Issue #3324 owns the first reusable synthetic Git/GitHub harness and populated
fixture families. Large logs are not fixture authority; retain only canonical
inputs and expected normalized observations here.
