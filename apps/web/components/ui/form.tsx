import type {
  ButtonHTMLAttributes,
  FormHTMLAttributes,
  InputHTMLAttributes,
  ReactNode,
  SelectHTMLAttributes,
  TextareaHTMLAttributes,
} from "react";
import { cn } from "@/lib/utils";

export function FormStack({
  children,
  className,
  ...props
}: FormHTMLAttributes<HTMLFormElement> & { children: ReactNode; className?: string }) {
  return (
    <form className={cn("flex flex-col gap-4", className)} {...props}>
      {children}
    </form>
  );
}

export function Field({
  children,
  hint,
  label,
  className,
}: {
  children: ReactNode;
  hint?: string;
  label: string;
  className?: string;
}) {
  return (
    <label className={cn("flex flex-col gap-1.5", className)}>
      <div className="flex justify-between items-center">
        <span className="text-xs font-bold text-muted-foreground uppercase tracking-wider">{label}</span>
        {hint && <span className="text-xs text-muted-foreground">{hint}</span>}
      </div>
      {children}
    </label>
  );
}

export function Input({ className, ...props }: InputHTMLAttributes<HTMLInputElement> & { className?: string }) {
  return (
    <input 
      className={cn(
        "flex h-9 w-full rounded-sm border border-border bg-input px-3 py-1 text-sm shadow-sm transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50", 
        className
      )} 
      {...props} 
    />
  );
}

export function Select({ className, ...props }: SelectHTMLAttributes<HTMLSelectElement> & { className?: string }) {
  return (
    <select 
      className={cn(
        "flex h-9 w-full rounded-sm border border-border bg-input px-3 py-1 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 appearance-none", 
        className
      )} 
      {...props} 
    />
  );
}

export function Textarea({ className, ...props }: TextareaHTMLAttributes<HTMLTextAreaElement> & { className?: string }) {
  return (
    <textarea 
      className={cn(
        "flex min-h-[60px] w-full rounded-sm border border-border bg-input px-3 py-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50", 
        className
      )} 
      {...props} 
    />
  );
}

export function ButtonRow({ children, className }: { children: ReactNode, className?: string }) {
  return <div className={cn("flex items-center gap-2", className)}>{children}</div>;
}

export function Button({
  children,
  className,
  tone = "primary",
  size = "default",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  children: ReactNode;
  className?: string;
  tone?: "primary" | "secondary" | "danger" | "ghost" | "outline";
  size?: "default" | "sm" | "lg" | "icon";
}) {
  return (
    <button
      className={cn(
        "inline-flex items-center justify-center whitespace-nowrap rounded-sm text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50",
        {
          "bg-primary text-primary-foreground shadow hover:bg-primary/90": tone === "primary",
          "bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80": tone === "secondary",
          "bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90": tone === "danger",
          "hover:bg-accent hover:text-accent-foreground": tone === "ghost",
          "border border-border bg-transparent shadow-sm hover:bg-accent hover:text-accent-foreground": tone === "outline",
        },
        {
          "h-9 px-4 py-2": size === "default",
          "h-8 rounded-sm px-3 text-xs": size === "sm",
          "h-10 rounded-sm px-8": size === "lg",
          "h-9 w-9": size === "icon",
        },
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
}
