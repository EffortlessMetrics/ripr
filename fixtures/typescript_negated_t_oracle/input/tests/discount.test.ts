test("discounted value is not the full amount", t => {
    t.not(applyDiscount(100, 100), 100);
});

test("discounted value is not deeply equal to the full amount", t => {
    t.notDeepEqual(applyDiscount(100, 100), 100);
});
