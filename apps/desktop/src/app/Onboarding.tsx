import { useState, type ReactNode } from "react";
import { ArrowRight, Lock, ShieldCheck, Trash2, type LucideIcon } from "lucide-react";
import { Badge, Button, PareMark, cx } from "@pare/ui";
import { t, useT } from "../i18n";
import { useApp } from "../state/app";

/** Built at render time so the copy follows the current language. */
function steps(): { icon: LucideIcon; title: string; body: ReactNode }[] {
  return [
    {
      icon: ShieldCheck,
      title: t("onboarding.labels.title"),
      body: (
        <ul className="flex flex-col gap-3 text-left">
          <li className="flex items-start gap-3">
            <Badge tone="safe">{t("tier.safe")}</Badge>
            <span className="text-body-m text-text-secondary">{t("onboarding.labels.safe")}</span>
          </li>
          <li className="flex items-start gap-3">
            <Badge tone="review">{t("tier.review")}</Badge>
            <span className="text-body-m text-text-secondary">{t("onboarding.labels.review")}</span>
          </li>
          <li className="flex items-start gap-3">
            <Badge tone="protected">{t("protect.label.keep")}</Badge>
            <span className="text-body-m text-text-secondary">{t("onboarding.labels.keep")}</span>
          </li>
        </ul>
      ),
    },
    {
      icon: Trash2,
      title: t("onboarding.bin.title"),
      body: <p className="text-body-l text-text-secondary">{t("onboarding.bin.body")}</p>,
    },
    {
      icon: Lock,
      title: t("onboarding.local.title"),
      body: null,
    },
  ];
}

export function Onboarding() {
  const { t } = useT();
  const update = useApp((s) => s.updateSettings);
  const [step, setStep] = useState(0);
  const [updates, setUpdates] = useState(false);
  const all = steps();
  const s = all[step]!;
  const last = step === all.length - 1;

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
            <p className="text-body-l text-text-secondary">{t("onboarding.local.body")}</p>
            <label className="flex cursor-pointer items-start gap-3 rounded-lg border border-border-default bg-bg-surface p-4 text-left">
              <input type="checkbox" checked={updates} onChange={(e) => setUpdates(e.currentTarget.checked)} className="mt-1 size-4 accent-[var(--pare-accent-default)]" />
              <span>
                <span className="block text-body-m-strong">{t("onboarding.updates.label")}</span>
                <span className="block text-body-s text-text-secondary">{t("onboarding.updates.hint")}</span>
              </span>
            </label>
          </div>
        )}
        <div className="flex items-center gap-2" aria-hidden="true">
          {all.map((_, i) => (
            <span key={i} className={cx("h-1.5 rounded-full transition-all", i === step ? "w-6 bg-accent-default" : "w-1.5 bg-border-strong")} />
          ))}
        </div>
        <div className="flex gap-3">
          {step > 0 && (
            <Button variant="secondary" onClick={() => setStep(step - 1)}>
              {t("onboarding.back")}
            </Button>
          )}
          {last ? (
            <Button size="l" onClick={() => void update({ onboardingDone: true, checkForUpdates: updates })}>
              {t("onboarding.start")}
            </Button>
          ) : (
            <Button size="l" icon={ArrowRight} onClick={() => setStep(step + 1)}>
              {t("onboarding.next")}
            </Button>
          )}
        </div>
        <p className="text-caption text-text-tertiary">{t("onboarding.step", { n: step + 1, total: all.length })}</p>
      </div>
    </div>
  );
}
