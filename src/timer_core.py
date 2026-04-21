from gi.repository import GLib


class TimerCore:
    TICK_INTERVAL_MS = 100

    def __init__(self):
        self._source_id = None
        self.remaining_seconds = 0
        self.total_seconds = 0
        self.is_running = False
        self.on_tick = None
        self.on_finished = None

    def start(self, seconds: int) -> None:
        self.stop()
        self.remaining_seconds = seconds
        self.total_seconds = seconds
        self.is_running = True
        self._schedule_tick()

    def resume(self) -> None:
        if self.remaining_seconds > 0 and not self.is_running:
            self.is_running = True
            self._schedule_tick()

    def pause(self) -> None:
        self.is_running = False
        self._cancel_tick()

    def stop(self) -> None:
        self.is_running = False
        self._cancel_tick()
        self.remaining_seconds = 0
        self.total_seconds = 0

    def add_seconds(self, seconds: int) -> None:
        self.remaining_seconds += seconds

    def _schedule_tick(self) -> None:
        self._cancel_tick()
        self._source_id = GLib.timeout_add(self.TICK_INTERVAL_MS, self._on_tick)

    def _cancel_tick(self) -> None:
        if self._source_id is not None:
            GLib.source_remove(self._source_id)
            self._source_id = None

    def _on_tick(self) -> bool:
        if not self.is_running:
            return False

        self.remaining_seconds -= self.TICK_INTERVAL_MS / 1000.0
        if self.remaining_seconds <= 0:
            self.remaining_seconds = 0
            self.is_running = False
            if self.on_tick:
                self.on_tick(self.remaining_seconds, self.total_seconds)
            if self.on_finished:
                self.on_finished()
            return False

        if self.on_tick:
            self.on_tick(self.remaining_seconds, self.total_seconds)
        return True