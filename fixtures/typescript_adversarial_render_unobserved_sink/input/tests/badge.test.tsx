import { Badge } from '../src/badge';

test('renders the cart label', () => {
  const html = Badge({ count: 5 });
  expect(html).toBe('<span class="badge">5</span>');
});
