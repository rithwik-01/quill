import * as React from "react";
import { Loader2 } from "lucide-react";

type Variant = "primary" | "secondary" | "ghost" | "destructive";
type Size = "sm" | "md" | "lg";

const variantCls: Record<Variant, string> = {
  primary: "bg-zinc-900 text-white hover:bg-zinc-800 disabled:bg-zinc-300 dark:bg-white dark:text-zinc-900 dark:hover:bg-zinc-100",
  secondary: "bg-zinc-100 text-zinc-900 hover:bg-zinc-200 border border-zinc-200 dark:bg-zinc-800 dark:text-zinc-100 dark:hover:bg-zinc-700 dark:border-zinc-700",
  ghost: "bg-transparent text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800",
  destructive: "bg-red-600 text-white hover:bg-red-700 disabled:bg-red-300",
};

const sizeCls: Record<Size, string> = {
  sm: "h-7 px-2.5 text-xs",
  md: "h-9 px-4 text-sm",
  lg: "h-10 px-6 text-sm",
};

export type ButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant;
  size?: Size;
  loading?: boolean;
};

export function Button({ variant = "primary", size = "md", loading = false, disabled, className = "", children, ...rest }: ButtonProps) {
  return (
    <button
      disabled={disabled || loading}
      className={`inline-flex items-center justify-center gap-1.5 rounded-lg font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-zinc-400 disabled:cursor-not-allowed disabled:opacity-50 ${variantCls[variant]} ${sizeCls[size]} ${className}`}
      {...rest}
    >
      {loading && <Loader2 className="h-4 w-4 animate-spin" />}
      {children}
    </button>
  );
}
