import { StyleDnaRadar } from '@/components/StyleDnaRadar';
import toast from 'react-hot-toast';

interface StyleDnaTabProps {
  storyId: string;
}

export function StyleDnaTab({ storyId }: StyleDnaTabProps) {
  return (
    <div className="max-w-2xl">
      <StyleDnaRadar
        storyId={storyId}
        onConstraintChange={constraints => {
          toast.success('StyleDNA 约束已更新');
        }}
      />
    </div>
  );
}
