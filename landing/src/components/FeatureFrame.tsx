interface FeatureFrameProps {
  src: string;
  alt: string;
}

export function FeatureFrame({ src, alt }: FeatureFrameProps) {
  return (
    <div className="overflow-hidden rounded-[2px] border border-ink-line bg-cream p-2 shadow-card transition-colors duration-200 hover:border-cinnabar">
      <img
        src={src}
        alt={alt}
        loading="lazy"
        className="w-full rounded-[2px] bg-ink-wash"
      />
    </div>
  );
}
