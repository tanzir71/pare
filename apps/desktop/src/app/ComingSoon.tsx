import { GitBranch, LayoutGrid } from "lucide-react";

const COPY = {
  projects: {
    icon: GitBranch,
    title: "Projects are coming soon",
    body: "Pare will clean node_modules, target and .venv only in git projects that are committed, pushed and idle.",
  },
  browse: {
    icon: LayoutGrid,
    title: "Browse is coming soon",
    body: "A storage map of whole drives, with the same safety labels and actions.",
  },
};

export function ComingSoon({ screen }: { screen: keyof typeof COPY }) {
  const c = COPY[screen];
  return (
    <div className="flex min-h-full flex-col items-center justify-center gap-3 px-8 text-center">
      <span className="flex size-14 items-center justify-center rounded-xl bg-bg-subtle text-text-secondary">
        <c.icon size={26} aria-hidden="true" />
      </span>
      <h1 className="text-heading-h2">{c.title}</h1>
      <p className="max-w-[440px] text-body-m text-text-secondary">{c.body}</p>
    </div>
  );
}
