interface StepCardProps {
  number: string;
  title: string;
  description: string;
}

export function StepCard({ number, title, description }: StepCardProps) {
  return (
    <div className="relative">
      <span className="mb-3 block font-display text-4xl text-cinnabar/30">{number}</span>
      <h3 className="mb-2 text-lg font-medium text-ink">{title}</h3>
      <p className="leading-relaxed text-charcoal">{description}</p>
    </div>
  );
}
