"""Stub for a heavy optional OSWorld evaluator dependency that is not installed on
the bench box. Importing works; using anything raises, so a task that needs it fails loudly."""
class _Missing:
    def __init__(self, name): self._n = name
    def __call__(self, *a, **k): raise RuntimeError(f"{self._n}: stubbed dependency, not installed on this box")
    def __getattr__(self, k): return _Missing(self._n + "." + k)
def __getattr__(name):
    if name.startswith("__"): raise AttributeError(name)
    return _Missing(__name__ + "." + name)
