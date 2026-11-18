import { GitBranch, LayoutGrid } from "lucide-react";
import { useT, type MessageKey } from "../i18n";

const COPY: Record<"projects" | "browse", { icon: typeof GitBranch; title: MessageKey; body: MessageKey }> = {
  projects: {
    icon: GitBranch,
    title: "comingSoon.projects.title",
    body: "comingSoon.projects.body",
  },
  browse: {
    icon: LayoutGrid,
    title: "comingSoon.browse.title",
    body: "comingSoon.browse.body",
  },
};

export function ComingSoon({ screen }: { screen: keyof typeof COPY }) {
  const { t } = useT();
  const c = COPY[screen];
  return (
    <div className="flex min-h-full flex-col items-center justify-center gap-3 px-8 text-center">
      <span className="flex size-14 items-center justify-center rounded-xl bg-bg-subtle text-text-secondary">
        <c.icon size={26} aria-hidden="true" />
      </span>
      <h1 className="text-heading-h2">{t(c.title)}</h1>
      <p className="max-w-[440px] text-body-m text-text-secondary">{t(c.body)}</p>
    </div>
  );
}
