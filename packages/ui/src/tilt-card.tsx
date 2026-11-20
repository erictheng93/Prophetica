"use client";

import {
  type CSSProperties,
  type HTMLAttributes,
  type MouseEvent,
  type ReactNode,
  useCallback,
  useRef,
  useState,
} from "react";
import { cn } from "./cn";

export interface TiltCardProps extends HTMLAttributes<HTMLDivElement> {
  children?: ReactNode;
  maxTilt?: number;
  spotlight?: boolean;
  spotlightColor?: string;
  className?: string;
  borderGlow?: boolean;
}

export function TiltCard({
  children,
  maxTilt = 6,
  spotlight = true,
  spotlightColor = "rgba(41, 151, 255, 0.12)",
  className,
  borderGlow = true,
  style,
  onMouseMove,
  onMouseLeave,
  ...props
}: TiltCardProps) {
  const cardRef = useRef<HTMLDivElement>(null);
  const [transform, setTransform] = useState<string>(
    "perspective(1000px) rotateX(0deg) rotateY(0deg) scale3d(1, 1, 1)",
  );
  const [spotlightPos, setSpotlightPos] = useState<{
    x: number;
    y: number;
    opacity: number;
  }>({
    x: 0,
    y: 0,
    opacity: 0,
  });

  const handleMouseMove = useCallback(
    (e: MouseEvent<HTMLDivElement>) => {
      if (!cardRef.current) return;
      const rect = cardRef.current.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      const centerX = rect.width / 2;
      const centerY = rect.height / 2;

      const rotateX = ((y - centerY) / centerY) * -maxTilt;
      const rotateY = ((x - centerX) / centerX) * maxTilt;

      setTransform(
        `perspective(1000px) rotateX(${rotateX.toFixed(2)}deg) rotateY(${rotateY.toFixed(2)}deg) scale3d(1.01, 1.01, 1.01)`,
      );
      setSpotlightPos({
        x,
        y,
        opacity: 1,
      });

      onMouseMove?.(e);
    },
    [maxTilt, onMouseMove],
  );

  const handleMouseLeave = useCallback(
    (e: MouseEvent<HTMLDivElement>) => {
      setTransform(
        "perspective(1000px) rotateX(0deg) rotateY(0deg) scale3d(1, 1, 1)",
      );
      setSpotlightPos((prev) => ({ ...prev, opacity: 0 }));
      onMouseLeave?.(e);
    },
    [onMouseLeave],
  );

  return (
    <div
      ref={cardRef}
      onMouseMove={handleMouseMove}
      onMouseLeave={handleMouseLeave}
      style={
        {
          transform,
          transition: "transform 0.25s cubic-bezier(0.2, 0, 0.2, 1)",
          transformStyle: "preserve-3d",
          willChange: "transform",
          ...style,
        } as CSSProperties
      }
      className={cn(
        "relative isolate overflow-hidden rounded-proph-lg border border-proph-hairline bg-proph-canvas transition-colors duration-200",
        borderGlow &&
          "hover:border-proph-primary/40 dark:hover:border-proph-primary/50",
        className,
      )}
      {...props}
    >
      {spotlight ? (
        <div
          aria-hidden="true"
          className="pointer-events-none absolute -inset-px z-10 rounded-[inherit] transition-opacity duration-300"
          style={{
            opacity: spotlightPos.opacity,
            background: `radial-gradient(400px circle at ${spotlightPos.x}px ${spotlightPos.y}px, ${spotlightColor}, transparent 70%)`,
          }}
        />
      ) : null}
      <div className="relative z-0 h-full w-full">{children}</div>
    </div>
  );
}
