import { useState } from "react";
import { ArrowRight, Lock, ShieldCheck, Trash2 } from "lucide-react";
import { Badge, Button, PareMark, cx } from "@pare/ui";
import { useApp } from "../state/app";

const STEPS = [
  {
    icon: ShieldCheck,
    title: "Every suggestion has a label",
    body: (
      <ul className="flex flex-col gap-3 text-left">
        <li className="flex items-start gap-3">
          <Badge tone="safe">Safe</Badge>
          <span className="text-body-m text-text-secondary">Rebuilds itself or re-downloads when needed: caches, build output, temp files.</span>
        </li>
        <li className="flex items-start gap-3">
          <Badge tone="review">Review</Badge>
          <span className="text-body-m text-text-secondary">Your own data, probably unused. Pare shows the dates so you can decide.</span>
        </li>
        <li className="flex items-start gap-3">
          <Badge tone="protected">Keep</Badge>
          <span className="text-body-m text-text-secondary">Used in the last 24 hours, open in an app, or uncommitted work. These can't be selected.</span>
        </li>
      </ul>
    ),
  },
  {
    icon: Trash2,
    title: "Your files go to the Recycle Bin first",
    body: (
      <p className="text-body-l text-text-secondary">
        Only data that rebuilds itself is deleted right away. Everything else goes to the Recycle Bin, so you can restore it from History. Pare empties
        only the items it moved, after 30 days (you can change that in Settings).
      </p>
    ),
  },
  {
    icon: Lock,
    title: "Nothing leaves your PC",
    body: null,
  },
];

export function Onboarding() {
  const update = useApp((s) => s.updateSettings);
  const [step, setStep] = useState(0);
  const [updates, setUpdates] = useState(false);
  const s = STEPS[step]!;
  const last = step === STEPS.length - 1;

  return (
    <div role="dialog" aria-modal="true" aria-labelledby="onboarding-title" className="fixed inset-0 z-50 flex items-center justify-center bg-bg-app p-8">
      <div className="flex w-full max-w-[560px] flex-col items-center gap-6 text-center">
        <PareMark size={48} />
        <span className="flex size-14 items-center justify-center rounded-xl bg-accent-subtle text-accent-on-subtle">
          <s.icon size={26} aria-hidden="true" />
        </span>
        <h1 id="onboarding-title" className="text-heading-h1">
          {s.title}
        </h1>
        {s.body ?? (
          <div className="flex flex-col gap-4">
            <p className="text-body-l text-text-secondary">
              No account, no telemetry. Scans and suggestions happen on this PC, and every action is logged locally in your AppData folder.
            </p>
            <label className="flex cursor-pointer items-start gap-3 rounded-lg border border-border-default bg-bg-surface p-4 text-left">
              <input type="checkbox" checked={updates} onChange={(e) => setUpdates(e.currentTarget.checked)} className="mt-1 size-4 accent-[var(--pare-accent-default)]" />
              <span>
                <span className="block text-body-m-strong">Check for updates once a day</span>
                <span className="block text-body-s text-text-secondary">The only time Pare connects to the internet: it asks GitHub whether a newer version exists. Off unless you turn it on.</span>
              </span>
            </label>
          </div>
        )}
        <div className="flex items-center gap-2" aria-hidden="true">
          {STEPS.map((_, i) => (
            <span key={i} className={cx("h-1.5 rounded-full transition-all", i === step ? "w-6 bg-accent-default" : "w-1.5 bg-border-strong")} />
          ))}
        </div>
        <div className="flex gap-3">
          {step > 0 && (
            <Button variant="secondary" onClick={() => setStep(step - 1)}>
              Back
            </Button>
          )}
          {last ? (
            <Button size="l" onClick={() => void update({ onboardingDone: true, checkForUpdates: updates })}>
              Start using Pare
            </Button>
          ) : (
            <Button size="l" icon={ArrowRight} onClick={() => setStep(step + 1)}>
              Next
            </Button>
          )}
        </div>
        <p className="text-caption text-text-tertiary">
          Step {step + 1} of {STEPS.length}
        </p>
      </div>
    </div>
  );
}
