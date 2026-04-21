from collections import Counter
from gi.repository import Adw, Gtk
from data_store import DataStore
from models import TrainingPlan


class HomePage(Adw.Bin):
    def __init__(self, data_store: DataStore, training_plan_page, **kwargs):
        super().__init__(**kwargs)
        self._store = data_store
        self._training_plan_page = training_plan_page
        self._on_switch_to_plans = None
        self._build_ui()

    def _build_ui(self):
        scrolled = Gtk.ScrolledWindow(vexpand=True)
        clamp = Adw.Clamp()
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=24)
        box.set_margin_top(36)
        box.set_margin_bottom(36)
        box.set_margin_start(24)
        box.set_margin_end(24)

        welcome = Gtk.Label(label="Welcome back!", css_classes=["title-1"])
        box.append(welcome)

        self._recent_card = Adw.PreferencesGroup(title="Continue Training")
        self._recent_row = None
        box.append(self._recent_card)

        self._recommended_card = Adw.PreferencesGroup(title="Recommended")
        self._recommended_row = None
        box.append(self._recommended_card)

        browse_btn = Gtk.Button(label="Browse Training Plans", css_classes=["pill"], halign=Gtk.Align.CENTER)
        browse_btn.connect("clicked", lambda _: self._go_to_plans())
        box.append(browse_btn)

        clamp.set_child(box)
        scrolled.set_child(clamp)
        self.set_child(scrolled)

    def refresh(self):
        self._update_recent()
        self._update_recommended()

    def _update_recent(self):
        if self._recent_row:
            self._recent_card.remove(self._recent_row)
            self._recent_row = None

        sessions = self._store.load_sessions()
        if not sessions:
            row = Adw.ActionRow(
                title="No training yet",
                subtitle="Start your first workout!",
            )
            go_btn = Gtk.Button(label="Browse", css_classes=["flat"])
            go_btn.connect("clicked", lambda _: self._go_to_plans())
            row.add_suffix(go_btn)
            self._recent_row = row
            self._recent_card.add(row)
            return

        last = sessions[0]
        plan = self._store.get_plan(last.plan_id)
        if not plan:
            plans = self._store.load_plans()
            plan = next((p for p in plans if p.name == last.plan_name), None)

        plan_name = plan.name if plan else last.plan_name
        date_str = last.started_at.strftime("%Y-%m-%d %H:%M")
        ex_count = len(last.exercises)

        row = Adw.ActionRow(
            title=plan_name,
            subtitle=f"Last: {date_str} | {ex_count} exercises",
        )

        if plan:
            start_btn = Gtk.Button(label="Start", css_classes=["suggested-action", "flat"])
            start_btn.connect("clicked", lambda _, p=plan: self._start_plan(p))
            row.add_suffix(start_btn)

        self._recent_row = row
        self._recent_card.add(row)

    def _update_recommended(self):
        if self._recommended_row:
            self._recommended_card.remove(self._recommended_row)
            self._recommended_row = None

        sessions = self._store.load_sessions()
        plans = self._store.load_plans()

        if not plans:
            row = Adw.ActionRow(title="No plans available")
            self._recommended_row = row
            self._recommended_card.add(row)
            return

        if sessions:
            plan_counts = Counter(s.plan_name for s in sessions)
            most_common_name, count = plan_counts.most_common(1)[0]
            recommended = next((p for p in plans if p.name == most_common_name), plans[0])
            subtitle = f"Completed {count} time{'s' if count != 1 else ''}"
        else:
            recommended = next((p for p in plans if p.name == "Full Body Beginner"), plans[0])
            subtitle = "Great for beginners"

        row = Adw.ActionRow(
            title=recommended.name,
            subtitle=subtitle,
        )
        start_btn = Gtk.Button(label="Start", css_classes=["suggested-action", "flat"])
        start_btn.connect("clicked", lambda _, p=recommended: self._start_plan(p))
        row.add_suffix(start_btn)

        self._recommended_row = row
        self._recommended_card.add(row)

    def _start_plan(self, plan: TrainingPlan):
        self._training_plan_page.start_plan(plan)

    def _go_to_plans(self):
        if self._on_switch_to_plans:
            self._on_switch_to_plans()