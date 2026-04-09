/**
 * Daily Metrics Bar Component
 * 
 * Displays summary statistics for the selected day including total active time,
 * idle time, AFK time, and utilization ratio. Each metric is clickable to
 * highlight corresponding timeline segments.
 */

"use client";

import { TrendingUp, TrendingDown } from "lucide-react";
import type { DailyMetrics } from "@/lib/activitywatchClient";
import { formatDuration } from "@/lib/activityProcessor";

interface DailyMetricsBarProps {
  metrics: DailyMetrics | null;
  comparisonMetrics?: DailyMetrics | null;
  onMetricClick?: (metricType: "active" | "idle" | "afk" | "utilization") => void;
}

export function DailyMetricsBar({ 
  metrics, 
  comparisonMetrics,
  onMetricClick,
}: DailyMetricsBarProps) {
  
  // Calculate comparison indicators
  const getComparison = (current: number, previous?: number) => {
    if (!previous || previous === 0) return null;
    const diff = ((current - previous) / previous) * 100;
    return {
      value: Math.abs(diff),
      isIncrease: diff > 0,
    };
  };

  const activeComparison = comparisonMetrics 
    ? getComparison(metrics?.total_active_seconds || 0, comparisonMetrics.total_active_seconds)
    : null;
  
  const idleComparison = comparisonMetrics
    ? getComparison(metrics?.total_idle_seconds || 0, comparisonMetrics.total_idle_seconds)
    : null;
  
  const afkComparison = comparisonMetrics
    ? getComparison(metrics?.total_afk_seconds || 0, comparisonMetrics.total_afk_seconds)
    : null;
  
  const utilizationComparison = comparisonMetrics
    ? getComparison(metrics?.utilization_ratio || 0, comparisonMetrics.utilization_ratio)
    : null;

  // Metric Card Component
  const MetricCard = ({
    label,
    value,
    bgColor,
    textColor,
    comparison,
    onClick,
  }: {
    label: string;
    value: string;
    bgColor: string;
    textColor: string;
    comparison?: { value: number; isIncrease: boolean } | null;
    onClick?: () => void;
  }) => (
    <div
      onClick={onClick}
      className={`${bgColor} rounded-[1rem] p-2 shadow-sm transition-all duration-200 ${
        onClick ? "cursor-pointer hover:shadow-md hover:scale-105" : ""
      }`}
    >
      <div className="flex flex-col">
        <div className={`text-xs font-medium ${textColor} opacity-80 mb-1`}>
          {label}
        </div>
        <div className={`text-2xl font-bold ${textColor}`}>
          {value}
        </div>
        {comparison && (
          <div className="flex items-center gap-1 mt-2">
            {comparison.isIncrease ? (
              <TrendingUp className={`w-4 h-4 ${textColor}`} />
            ) : (
              <TrendingDown className={`w-4 h-4 ${textColor}`} />
            )}
            <span className={`text-xs ${textColor} font-medium`}>
              {comparison.value.toFixed(1)}%
            </span>
          </div>
        )}
      </div>
    </div>
  );

  if (!metrics) {
    const ghostCards = [
      { bgColor: "bg-green-50", shimmerColor: "bg-green-200" },
      { bgColor: "bg-yellow-50", shimmerColor: "bg-yellow-200" },
      { bgColor: "bg-red-50", shimmerColor: "bg-red-200" },
      { bgColor: "bg-blue-50", shimmerColor: "bg-blue-200" },
    ];

    return (
      <div className="grid grid-cols-2 gap-2">
        {ghostCards.map((card, i) => (
          <div
            key={i}
            className={`${card.bgColor} rounded-[1rem] p-2 shadow-sm animate-pulse`}
          >
            <div className="flex flex-col">
              <div className={`h-3 ${card.shimmerColor} rounded w-16 mb-1`}></div>
              <div className={`h-7 ${card.shimmerColor} rounded w-12`}></div>
            </div>
          </div>
        ))}
      </div>
    );
  }

  return (
    <div className="grid grid-cols-2 gap-2">
      {/* Total Active Time */}
      <MetricCard
        label="Total Active"
        value={formatDuration(metrics.total_active_seconds, "compact")}
        bgColor="bg-green-50"
        textColor="text-green-700"
        comparison={activeComparison}
        onClick={() => onMetricClick?.("active")}
      />

      {/* Total Idle Time */}
      <MetricCard
        label="Total Idle"
        value={formatDuration(metrics.total_idle_seconds, "compact")}
        bgColor="bg-yellow-50"
        textColor="text-yellow-700"
        comparison={idleComparison}
        onClick={() => onMetricClick?.("idle")}
      />

      {/* Total AFK Time */}
      <MetricCard
        label="Total AFK"
        value={formatDuration(metrics.total_afk_seconds, "compact")}
        bgColor="bg-red-50"
        textColor="text-red-700"
        comparison={afkComparison}
        onClick={() => onMetricClick?.("afk")}
      />

      {/* Utilization Ratio */}
      <MetricCard
        label="Utilization"
        value={`${(metrics.utilization_ratio * 100).toFixed(1)}%`}
        bgColor="bg-blue-50"
        textColor="text-blue-700"
        comparison={utilizationComparison}
        onClick={() => onMetricClick?.("utilization")}
      />
    </div>
  );
}

