import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Database } from 'lucide-react';
import { BoolInput, NumberInput, TextInput, SelectInput, Row, SectionCard } from './form-bits';

// @nubase/ui Card/CardContent are only structural here — stub them to keep the test isolated.
vi.mock('@nubase/ui', () => ({
  Card: ({ children }: { children: React.ReactNode }) => <div data-testid="card">{children}</div>,
  CardContent: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));

describe('form-bits', () => {
  it('BoolInput reflects value and emits the toggled value', () => {
    const onChange = vi.fn();
    render(<BoolInput value={false} onChange={onChange} />);
    const box = screen.getByRole('checkbox') as HTMLInputElement;
    expect(box.checked).toBe(false);
    fireEvent.click(box);
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('NumberInput parses to a number and ignores non-numeric', () => {
    const onChange = vi.fn();
    render(<NumberInput value={5} onChange={onChange} />);
    const input = screen.getByRole('spinbutton');
    fireEvent.change(input, { target: { value: '8' } });
    expect(onChange).toHaveBeenCalledWith(8);
  });

  it('TextInput emits the raw string', () => {
    const onChange = vi.fn();
    render(<TextInput value="" onChange={onChange} placeholder="x" />);
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'hello' } });
    expect(onChange).toHaveBeenCalledWith('hello');
  });

  it('SelectInput renders options and emits the chosen value', () => {
    const onChange = vi.fn();
    render(<SelectInput value="a" onChange={onChange} options={['a', 'b', 'c']} />);
    const select = screen.getByRole('combobox');
    expect(screen.getAllByRole('option')).toHaveLength(3);
    fireEvent.change(select, { target: { value: 'b' } });
    expect(onChange).toHaveBeenCalledWith('b');
  });

  it('Row renders its label and children', () => {
    render(<Row label="My Label"><span>child</span></Row>);
    expect(screen.getByText('My Label')).toBeInTheDocument();
    expect(screen.getByText('child')).toBeInTheDocument();
  });

  it('SectionCard renders title, description and children', () => {
    render(
      <SectionCard icon={Database} title="Sec" description="desc">
        <div>body</div>
      </SectionCard>,
    );
    expect(screen.getByText('Sec')).toBeInTheDocument();
    expect(screen.getByText('desc')).toBeInTheDocument();
    expect(screen.getByText('body')).toBeInTheDocument();
  });
});
