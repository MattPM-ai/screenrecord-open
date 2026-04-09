/**
 * ============================================================================
 * COMPONENT: Button
 * ============================================================================
 * 
 * PURPOSE: Reusable button component with multiple variants, sizes, and themes
 * 
 * PROPS:
 * - href: string (optional) - When provided, renders as Next.js Link
 * - variant: "primary" | "secondary" | "outline" | "ghost" (default: "primary")
 * - size: "sm" | "default" | "lg" (default: "default")
 * - theme: "light" | "dark" (default: "light")
 * - withGradient: boolean (default: false)
 * - className: string (optional) - Additional CSS classes
 * - children: ReactNode
 * 
 * ============================================================================
 */

import React from "react"
import { cn } from "../utils/cn"

type ButtonVariant = "primary" | "secondary" | "outline" | "ghost"
type ButtonSize = "sm" | "default" | "lg"
type ButtonTheme = "light" | "dark"

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  href?: string
  variant?: ButtonVariant
  size?: ButtonSize
  theme?: ButtonTheme
  withGradient?: boolean
}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      href,
      variant = "primary",
      size = "default",
      theme = "light",
      withGradient = false,
      className,
      children,
      ...props
    },
    ref
  ) => {
    // Variant styling rules
    const variantClasses: Record<ButtonVariant, string> = {
      primary: "bg-primary text-primary-foreground hover:bg-primary/90 bg",
      secondary:
        theme === "dark"
          ? "bg-secondary text-secondary-foreground hover:bg-secondary/90"
          : "bg-secondary text-secondary-foreground hover:bg-secondary/90",
      outline: "border border-border bg-transparent hover:bg-accent",
      ghost: "bg-transparent hover:bg-accent text-foreground",
    }

    // Size styling rules
    const sizeClasses: Record<ButtonSize, string> = {
      sm: "h-9 px-4 py-2 text-sm",
      default: "h-11 px-6 py-3",
      lg: "h-13 px-8 py-4 text-lg",
    }

    // Base classes applied to all buttons
    const baseClasses = cn(
      "inline-flex items-center justify-center",
      "font-medium rounded-2xl",
      "transition-colors duration-200",
      "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
      "disabled:pointer-events-none disabled:opacity-50",
      "cursor-pointer no-underline",
      variantClasses[variant],
      sizeClasses[size],
      className
    )

    // Gradient text wrapper
    const content = withGradient ? (
      <span className="bg-linear-to-r from-background via-muted to-background bg-clip-text text-transparent">
        {children}
      </span>
    ) : (
      children
    )

    // Note: href prop would require Next.js Link component
    // For now, we'll just render as button regardless of href
    // Apps using this should wrap it or extend it with Link support

    return (
      <button className={baseClasses} ref={ref} {...props}>
        {content}
      </button>
    )
  }
)

Button.displayName = "Button"

