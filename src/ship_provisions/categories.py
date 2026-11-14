"""Rule-based categorization of provision line items."""

from __future__ import annotations

# (category, keywords) - first match wins, order matters.
_RULES: list[tuple[str, tuple[str, ...]]] = [
    ("Bonded store", ("CIGARETTE", "TOBACCO", "BONDED")),
    ("Cleaning & cabin", (
        "WASHINGPOWDER", "WASHINGLIQUID", "SOFTENER", "DISHWASH", "DISH WASH",
        "DETERGENT", "SOAP", "CLEANER", "DESINFECT", "DESCALER", "ONTSTOPPER",
        "GARBAGE", "BAGS", "FOIL", "PAPER", "NAPKIN", "TOILET", "GLOVES",
        "SPONGE", "SCRUB", "MOP", "DUSTER", "STIRRER", "CUP", "CUTLERY",
        "SANITAIR", "HYGIENE", "BLEACH", "AJAX", "DREFT", "OMO", "ROBIJN",
        "DISHWASHING", "BAKING PAPER", "ALUMINIUM FOIL", "CLING",
    )),
    ("Clothing & rags", ("RAGS", "COOK'S JACKET", "CHINO", "WORKING CLOTHES", "OVEN GLOVES")),
    ("Kitchen equipment", ("PAN", "GRATER", "STRAINER", "GASTRONORM", "WOK", "PIPING TIPES", "OPBERGBOX", "CUTLERY TRAY")),
    ("Transport & services", ("TRANSPORT",)),
    ("Meat & poultry", (
        "BEEF", "VEAL", "PORK", "BACON", "HAM", "SAUSAGE", "CHORIZO", "SALAMI",
        "PROSCIUTTO", "SERRANO", "CHICKEN", "TURKEY", "DUCK", "LAMB", "MINCED MEAT",
        "TENDERLOIN", "STRIPLOIN", "SPARE RIBS", "PORKNECK", "OSOBUCCO", "KEBAB",
        "MINCED MEAT ROLLS", "CROQUETTES", "GYRO", "MEAT SPICES", "MINCED MEAT SPICES",
        "STEAK", "BURGER", "HOT SHOTS", "VLAMMETJES",
    )),
    ("Fish & seafood", (
        "SALMON", "TUNA", "PRAWNS", "ANCHOVY", "POLLACK", "COD", "SOLE",
        "FISH", "SHRIMP", "MUSSEL", "SCampi",
    )),
    ("Dairy & eggs", (
        "CHEESE", "BUTTER", "EGG", "YOGHURT", "YOGURT", "MILK", "CREAM",
        "MASCARPONE", "MOZZARELLA", "COTTAGE", "KEFIR", "QUARK", "DANIO",
        "CUSTARD", "FETA", "GRANO", "CHEDDAR", "GOUDA", "BRIE", "ACTIMEL",
        "KWARK", "SPREADCHEESE",
    )),
    ("Plant milk & tofu", ("TOFU", "ALMOND MILK", "SOY MILK", "SOYA MILK", "OAT MILK", "COCOSMILK", "CHOCOMEL")),
    ("Fresh vegetables & herbs", (
        "POTATO", "EGGPLANT", "CAULIFLOWER", "BROCCOLI", "ZUCHINI", "DILL",
        "LETTUCE", "GARLIC", "PEPPER", "PARSLEY", "LEEK", "BEETROOT", "CHILI",
        "BEANS", "TOMATO", "ONION", "CARROT", "CHICORY", "CABBAGE", "BASIL",
        "ASPARAGUS", "MUSHROOM", "CUCUMBER", "CORIANDER", "CELERY", "GINGER",
        "SPINACH", "AVOCADO", "THYME", "MINT", "ROSEMARY", "PUMPKIN", "CORN",
        "PEAS", "KNOLSELDERIJ", "LIME", "LEMONS", "ARTICHOKE",
    )),
    ("Fresh fruit", (
        "STRAWBERR", "BANANA", "GRAPES", "MELON", "KIWI", "MANDARIN", "PEAR",
        "ORANGE", "LEMON", "PLUM", "MANGO", "PINEAPPLE", "DRAGON FRUIT",
        "BLUEBERR", "RASPBERR", "CHERRIES", "APPLE", "DATES", "LIME",
    )),
    ("Frozen food", ("FROZEN", "FRITES", "FRIES", "POMMES", "PATAT", "SPRING ROLL", "ONION RINGS")),
    ("Ice cream & frozen desserts", ("ICE CREAM", "ICECREAM", "MAGNUM", "CORNETTO", "ICE CREAM")),
    ("Canned & jarred goods", (
        "TIN", "JAR", "OLIVES", "BEANS IN TOMATO", "PEELED", "CHOPPED",
        "CORN KERNELS", "MIXED FRUIT", "ARTICHOKE", "SUNDRIED", "PICKLED",
        "CHUTNEY", "COMPOTE", "CUCUMBER",
    )),
    ("Sauces & condiments", (
        "SAUCE", "KETCHUP", "MAYONAISE", "MAYONNAISE", "MUSTARD", "DRESSING",
        "SOYA", "SOYASAUCE", "KETJAP", "WORCESTER", "TABASCO", "OISTER", "OYSTER",
        "HOI SIN", "SATE", "SATAY", "SAMBAL", "HOT SAUCE", "CHILI SAUCE", "TARTARE",
        "GARLIC SAUCE", "BBQ", "BARBECUE", "HONIG", "HONEY", "JAM", "SPAGHERONI",
        "CURRY", "ROUX", "BOUILLON", "AROMAT",
    )),
    ("Spices & seasoning", (
        "SPICE", "PEPPER", "SALT", "SESAME", "CUMIN", "NUTMEG", "GINGER GROUND",
        "SEASONING", "VANILLE", "EXTRACT", "THAI", "MASSALA", "KERRIE", "SPICEMIX",
        "BAYMAN",
    )),
    ("Oils & fats", ("OIL", "FAT", "CROMA", "MARGARINE")),
    ("Pasta, rice & noodles", (
        "PASTA", "SPAGHETTI", "SPAGHERONI", "MACARONI", "PENNE", "LINGUINE",
        "TORTELLINI", "RAVIOLI", "NOODLE", "NOODLE-MIX", "RICE", "BASMATI",
        "TORTILLA", "NAAN", "PITA",
    )),
    ("Dry goods & baking", (
        "FLOUR", "SUGAR", "OATMEAL", "CRUESLI", "CEREAL", "MUESLI", "CRACKER",
        "BISCUIT", "COOKIE", "WAFeLS", "WAFELS", "DIGESTIVE", "OAT", "LENTILS",
        "PEAS", "PEANUTS", "NUT", "CACAO", "CAKEMIX", "BATTERMIX", "BREAD CRUMBS",
        "CHOCOLATE", "RAISINS", "MAIZENA", "BAKED BEANS", "DRY GREEN",
    )),
    ("Bakery & bread", ("BREAD", "BUN", "BAGUETTE", "ROLL", "DONUT", "PASTRY", "STROOPWAFEL", "YORKSHIRE")),
    ("Sweets & snacks", (
        "CHOCOLATE", "CANDY", "GUMS", "TOBLERONE", "TWIX", "SNICKERS", "BOUNTY",
        "MARS", "CHIPS", "LAY'S", "LAYS", "OREO", "DROP", "LICORICE", "HARLEQUIN",
        "DONUT", "LADY FINGERS", "PENNY",
    )),
    ("Coffee & tea", ("COFFEE", "NESCAFE", "LAVAZZA", "DOUWE", "TEA BAG", "TEA ", "ROOIBOS", "EARL GREY", "CAMOMILE", "YORKSHIRE TEA")),
    ("Drinks", (
        "COLA", "PEPSI", "FANTA", "TONIC", "SODA", "ICE TEA", "JUICE", "DRINK",
        "GUAVA", "CRANBERRY", "PEACH", "WINE CHARDONNAY", "SOFT DRINK",
    )),
    ("Water", ("WATER",)),
]


def categorize(description: str, invoice_reference: str = "") -> str:
    ref = (invoice_reference or "").upper()
    if ref == "BONDED":
        return "Bonded store"
    if ref in ("CABIN",):
        return "Cleaning & cabin"
    if ref in ("WORKING CLOTHES", "COTTON RAGS", "RAGS"):
        return "Clothing & rags"
    if ref == "PROVISIONS":
        pass  # fall through to keyword rules
    text = description.upper()
    for category, keywords in _RULES:
        if category in ("Bonded store", "Cleaning & cabin", "Clothing & rags",
                        "Kitchen equipment", "Transport & services"):
            continue  # handled via reference / special rules below
        for kw in keywords:
            if kw in text:
                return category
    # special keyword-only rules
    for category, keywords in _RULES:
        if category in ("Transport & services", "Kitchen equipment"):
            for kw in keywords:
                if kw in text:
                    return category
    return "Other"


def normalize_name(description: str) -> str:
    return " ".join(description.upper().split())
