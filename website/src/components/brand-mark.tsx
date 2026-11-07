type BrandMarkProps = {
  className?: string;
};

export function BrandMark({ className }: BrandMarkProps) {
  return (
    <svg className={className} viewBox="0 0 1024 1024" aria-hidden="true">
      <rect width="1024" height="1024" rx="230" fill="#111418" />
      <rect x="124" y="164" width="522" height="696" rx="50" fill="#F4F1E8" />
      <rect x="196" y="467" width="373" height="90" rx="45" fill="#1B1E22" />
      <circle cx="810" cy="512" r="77" fill="#F2C230" />
    </svg>
  );
}
