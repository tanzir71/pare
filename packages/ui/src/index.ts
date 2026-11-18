// @pare/ui — presentational React components shared by apps/web (rendered to static HTML)
// and apps/desktop. Style comes only from token classes (@pare/tokens); consumers must load
// tokens.css + tailwindcss + theme.css and point Tailwind's @source at this package's src/.

export { cx, focusRing } from "./cx";
export { formatBytes, formatBytesExact } from "./format";

export { PareMark, type PareMarkProps } from "./PareMark";
export { Lockup, type LockupProps } from "./Lockup";
export {
  Button,
  buttonClassName,
  type ButtonProps,
  type ButtonAsButtonProps,
  type ButtonAsLinkProps,
  type ButtonSize,
  type ButtonVariant,
} from "./Button";
export { Badge, type BadgeProps, type BadgeTone } from "./Badge";
export { Checkbox, type CheckboxProps } from "./Checkbox";
export {
  Chip,
  chipClassName,
  type ChipProps,
  type ChipButtonProps,
  type ChipStaticProps,
} from "./Chip";
export { NavItem, type NavItemProps } from "./NavItem";
export {
  CategoryTile,
  hueClasses,
  type CategoryHue,
  type CategoryTileProps,
  type CategoryTileSize,
} from "./CategoryTile";
export { SuggestionCard, type SuggestionCardProps } from "./SuggestionCard";
export { FileRow, type FileRowProps } from "./FileRow";
export { TitleBar, type TitleBarProps } from "./TitleBar";
export { Sidebar, type SidebarProps, type SidebarDrive, type SidebarScreen } from "./Sidebar";
export { StorageSummary, type StorageSummaryProps } from "./StorageSummary";
export { FeatureCard, type FeatureCardProps } from "./FeatureCard";
export { SearchField, type SearchFieldProps } from "./SearchField";
