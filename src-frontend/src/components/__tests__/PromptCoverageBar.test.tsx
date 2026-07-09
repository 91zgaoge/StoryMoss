import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { PromptCoverageBar, extractPromptCoverage } from '../PromptCoverageBar';

describe('PromptCoverageBar', () => {
  it('renders filled/total from details', () => {
    render(
      <PromptCoverageBar
        details={{
          contract_redlines: true,
          core_characters: true,
          related_entity_summaries: false,
          filled_slots: 2,
          total_slots: 10,
        }}
      />
    );
    expect(screen.getByTestId('prompt-coverage-bar')).toBeInTheDocument();
    expect(screen.getByText(/2\/10/)).toBeInTheDocument();
    expect(screen.getByText('合同红线')).toBeInTheDocument();
    expect(screen.getByText('KG摘要')).toBeInTheDocument();
  });

  it('extractPromptCoverage finds prompt_coverage step', () => {
    const cov = extractPromptCoverage([
      { name: 'writer', details: { foo: 1 } },
      {
        name: 'prompt_coverage',
        details: { filled_slots: 4, total_slots: 10, core_characters: true },
      },
    ]);
    expect(cov?.filled_slots).toBe(4);
    expect(cov?.core_characters).toBe(true);
  });

  it('extractPromptCoverage returns null when missing', () => {
    expect(extractPromptCoverage([{ name: 'writer' }])).toBeNull();
  });
});
