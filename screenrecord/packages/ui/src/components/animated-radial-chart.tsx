"use client";
import { motion, useMotionValue, useTransform, animate, useInView } from "framer-motion"
import { useEffect, useRef } from "react"
import { cn } from "../utils/cn"

interface AnimatedRadialChartProps {
  value?: number
  size?: number
  strokeWidth?: number
  className?: string
  showLabels?: boolean
  duration?: number
}
export function AnimatedRadialChart({
  value = 74,
  size = 300,
  strokeWidth: customStrokeWidth,
  className,
  showLabels = true,
  duration = 2
}: AnimatedRadialChartProps) {
  // Viewport detection for scroll-based animation
  const containerRef = useRef(null)
  const isInView = useInView(containerRef, { once: true, amount: 0.5 })

  // Dynamic stroke width based on size if not provided
  const strokeWidth = customStrokeWidth ?? Math.max(12, size * 0.06)
  const radius = size * 0.35
  const center = size / 2
  const circumference = Math.PI * radius

  // Calculate inner line radius (4px inside the main arc)
  const innerLineRadius = radius - strokeWidth - 4

  // Motion values for animation
  const animatedValue = useMotionValue(0)
  const offset = useTransform(animatedValue, [0, 100], [circumference, 0])

  // Calculate animated positions
  const progressAngle = useTransform(animatedValue, [0, 100], [-Math.PI, 0])
  const innerRadius = radius - strokeWidth / 2

  // Animate to the target value when component enters viewport
  useEffect(() => {
    if (!isInView) return

    const controls = animate(animatedValue, value, {
      duration,
      ease: "easeOut",
    })

    return controls.stop
  }, [value, animatedValue, duration, isInView])

  // Helper function: Convert RGB to hex
  const rgbToHex = (r: number, g: number, b: number) => {
    const toHex = (n: number) => {
      const hex = Math.round(Math.max(0, Math.min(255, n))).toString(16)
      return hex.length === 1 ? '0' + hex : hex
    }
    return `#${toHex(r)}${toHex(g)}${toHex(b)}`
  }

  // Helper function: Interpolate between two colors
  const interpolateColor = (color1: { r: number; g: number; b: number }, color2: { r: number; g: number; b: number }, factor: number) => {
    const r = color1.r + (color2.r - color1.r) * factor
    const g = color1.g + (color2.g - color1.g) * factor
    const b = color1.b + (color2.b - color1.b) * factor
    return { r, g, b }
  }

  // Calculate gradient colors based on percentage
  const calculateGradientColors = (percentage: number) => {
    // Define color waypoints
    const red = { r: 255, g: 0, b: 0 }
    const yellow = { r: 255, g: 255, b: 0 }
    const green = { r: 0, g: 255, b: 0 }

    let baseColor
    if (percentage <= 50) {
      // Transition from red to yellow (0-50%)
      const factor = percentage / 50
      baseColor = interpolateColor(red, yellow, factor)
    } else {
      // Transition from yellow to green (50-100%)
      const factor = (percentage - 50) / 50
      baseColor = interpolateColor(yellow, green, factor)
    }

    // Create three stops for smooth gradient with slight variations
    const darken = (color: { r: number; g: number; b: number }, amount: number) => ({
      r: color.r * (1 - amount),
      g: color.g * (1 - amount),
      b: color.b * (1 - amount)
    })

    const lighten = (color: { r: number; g: number; b: number }, amount: number) => ({
      r: Math.min(255, color.r + (255 - color.r) * amount),
      g: Math.min(255, color.g + (255 - color.g) * amount),
      b: Math.min(255, color.b + (255 - color.b) * amount)
    })

    // Generate three color stops with variations
    const stop1 = baseColor
    const stop2 = lighten(baseColor, 0.15)
    const stop3 = darken(baseColor, 0.15)

    return {
      color1: rgbToHex(stop1.r, stop1.g, stop1.b),
      color2: rgbToHex(stop2.r, stop2.g, stop2.b),
      color3: rgbToHex(stop3.r, stop3.g, stop3.b)
    }
  }

  // Dynamic color values based on animated percentage
  const gradientColors = useTransform(animatedValue, (val) => calculateGradientColors(val))
  const color1 = useTransform(gradientColors, (colors) => colors.color1)
  const color2 = useTransform(gradientColors, (colors) => colors.color2)
  const color3 = useTransform(gradientColors, (colors) => colors.color3)

  // Calculate responsive font size
  const fontSize = Math.max(16, size * 0.1)
  const labelFontSize = Math.max(12, size * 0.04)

  return (
    <div
      ref={containerRef}
      className={cn("relative", className)}
      style={{ width: size, height: size * 0.7 }}>
      <svg
        width={size}
        height={size * 0.7}
        viewBox={`0 0 ${size} ${size * 0.7}`}
        className="overflow-visible">
        <defs>
          {/* Base track gradient - adapts to light/dark mode */}
          <linearGradient id={`baseGradient-${size}`} x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stopColor="hsl(var(--muted-foreground))" stopOpacity="0.3" />
            <stop offset="50%" stopColor="hsl(var(--muted-foreground))" stopOpacity="0.2" />
            <stop offset="100%" stopColor="hsl(var(--muted-foreground))" stopOpacity="0.15" />
          </linearGradient>

          {/* Progress gradient - dynamic red to green based on percentage */}
          <linearGradient id={`progressGradient-${size}`} x1="0%" y1="0%" x2="100%" y2="0%">
            <motion.stop offset="0%" stopColor={color1} />
            <motion.stop offset="50%" stopColor={color2} />
            <motion.stop offset="100%" stopColor={color3} />
          </linearGradient>

          {/* Text gradient - adapts to light/dark mode */}
          <linearGradient id={`textGradient-${size}`} x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="hsl(var(--muted-foreground))" stopOpacity="0.7" />
            <stop offset="50%" stopColor="hsl(var(--muted-foreground))" stopOpacity="0.5" />
            <stop offset="100%" stopColor="hsl(var(--muted-foreground))" stopOpacity="0.3" />
          </linearGradient>

          {/* Drop shadow filter */}
          <filter id={`dropshadow-${size}`} x="-50%" y="-50%" width="200%" height="200%">
            <feDropShadow dx="0" dy="2" stdDeviation="3" floodColor="#000000" floodOpacity="0.3" />
          </filter>
        </defs>

        {/* Inner thin line (1px light gray) */}
        <path
          d={`M ${center - innerLineRadius} ${center} A ${innerLineRadius} ${innerLineRadius} 0 0 1 ${center + innerLineRadius} ${center}`}
          fill="none"
          stroke="#6b7280"
          strokeWidth="1"
          strokeLinecap="butt"
          opacity="0.6" />

        {/* Base track */}
        <path
          d={`M ${center - radius} ${center} A ${radius} ${radius} 0 0 1 ${center + radius} ${center}`}
          fill="none"
          stroke={`url(#baseGradient-${size})`}
          strokeWidth={strokeWidth}
          strokeLinecap="butt"
          filter={`url(#dropshadow-${size})`} />

        {/* Animated Progress track */}
        <motion.path
          d={`M ${center - radius} ${center} A ${radius} ${radius} 0 0 1 ${center + radius} ${center}`}
          fill="none"
          stroke={`url(#progressGradient-${size})`}
          strokeWidth={strokeWidth}
          strokeLinecap="butt"
          strokeDasharray={circumference}
          strokeDashoffset={offset}
          filter={`url(#dropshadow-${size})`} />

        {/* Animated extending line */}
        <motion.line
          x1={useTransform(progressAngle, (angle) => center + Math.cos(angle) * innerRadius)}
          y1={useTransform(progressAngle, (angle) => center + Math.sin(angle) * innerRadius)}
          x2={useTransform(
            progressAngle,
            (angle) => center + Math.cos(angle) * innerRadius - Math.cos(angle) * 30
          )}
          y2={useTransform(
            progressAngle,
            (angle) => center + Math.sin(angle) * innerRadius - Math.sin(angle) * 30
          )}
          stroke={`url(#textGradient-${size})`}
          strokeWidth="1"
          strokeLinecap="butt" />
      </svg>
      {/* Animated center percentage display with gradient text */}
      <div className="absolute inset-0 flex items-center justify-center">
        <motion.div
          className="font-bold tracking-tight mt-8"
          style={{ fontSize: `${fontSize}px` }}
          initial={{ opacity: 0, scale: 0.5 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ duration: 0.5, delay: duration * 0.2 }}>
          <span
            style={{
              background: "linear-gradient(to right, var(--muted-foreground), #9ca3af)",
              WebkitBackgroundClip: "text",
              WebkitTextFillColor: "transparent",
              backgroundClip: "text",
            }}>
            <motion.span>{useTransform(animatedValue, (latest) => Math.round(latest))}</motion.span>
          </span>
        </motion.div>
      </div>
      {/* 0% and 100% labels */}
      {showLabels && (
        <>
          <motion.div
            className="absolute text-muted-foreground font-medium"
            style={{
              fontSize: `${labelFontSize}px`,
              left: center - radius - 5,
              top: center + strokeWidth / 2,
            }}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5, delay: duration * 0.25 }}>
            0
          </motion.div>
          <motion.div
            className="absolute text-muted-foreground font-medium"
            style={{
              fontSize: `${labelFontSize}px`,
              left: center + radius - 12.5,
              top: center + strokeWidth / 2,
            }}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5, delay: duration * 0.25 }}>
            100
          </motion.div>
        </>
      )}
    </div>
  );
}
