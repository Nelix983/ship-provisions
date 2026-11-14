//! Rule-based categorization of provision line items.

pub const RULES: &[(&str, &[&str])] = &[
    ("Meat & poultry", &[
        "BEEF", "VEAL", "PORK", "BACON", "HAM", "SAUSAGE", "CHORIZO", "SALAMI",
        "PROSCIUTTO", "SERRANO", "CHICKEN", "TURKEY", "DUCK", "LAMB", "MINCED MEAT",
        "TENDERLOIN", "STRIPLOIN", "SPARE RIBS", "PORKNECK", "OSOBUCCO", "KEBAB",
        "CROQUETTES", "MEAT SPICES", "STEAK", "BURGER", "HOT SHOTS", "VLAMMETJES",
    ]),
    ("Fish & seafood", &[
        "SALMON", "TUNA", "PRAWNS", "ANCHOVY", "POLLACK", "COD", "SOLE",
        "FISH", "SHRIMP", "MUSSEL",
    ]),
    ("Dairy & eggs", &[
        "CHEESE", "BUTTER", "EGG", "YOGHURT", "YOGURT", "MILK", "CREAM",
        "MASCARPONE", "MOZZARELLA", "COTTAGE", "KEFIR", "QUARK", "DANIO",
        "CUSTARD", "FETA", "GRANO", "CHEDDAR", "GOUDA", "BRIE", "ACTIMEL",
        "KWARK", "SPREADCHEESE",
    ]),
    ("Plant milk & tofu", &["TOFU", "ALMOND MILK", "SOY MILK", "SOYA MILK", "OAT MILK", "COCOSMILK", "CHOCOMEL"]),
    ("Fresh vegetables & herbs", &[
        "POTATO", "EGGPLANT", "CAULIFLOWER", "BROCCOLI", "ZUCHINI", "DILL",
        "LETTUCE", "GARLIC", "PEPPER", "PARSLEY", "LEEK", "BEETROOT", "CHILI",
        "BEANS", "TOMATO", "ONION", "CARROT", "CHICORY", "CABBAGE", "BASIL",
        "ASPARAGUS", "MUSHROOM", "CUCUMBER", "CORIANDER", "CELERY", "GINGER",
        "SPINACH", "AVOCADO", "THYME", "MINT", "ROSEMARY", "PUMPKIN", "CORN",
        "PEAS", "KNOLSELDERIJ", "LIME", "ARTICHOKE",
    ]),
    ("Fresh fruit", &[
        "STRAWBERR", "BANANA", "GRAPES", "MELON", "KIWI", "MANDARIN", "PEAR",
        "ORANGE", "LEMON", "PLUM", "MANGO", "PINEAPPLE", "DRAGON FRUIT",
        "BLUEBERR", "RASPBERR", "CHERRIES", "APPLE", "DATES",
    ]),
    ("Frozen food", &["FROZEN", "FRITES", "FRIES", "POMMES", "PATAT", "SPRING ROLL", "ONION RINGS"]),
    ("Ice cream & frozen desserts", &["ICE CREAM", "ICECREAM", "MAGNUM", "CORNETTO"]),
    ("Canned & jarred goods", &[
        "TIN", "JAR", "OLIVES", "BEANS IN TOMATO", "PEELED", "CHOPPED",
        "CORN KERNELS", "MIXED FRUIT", "ARTICHOKE", "SUNDRIED", "PICKLED",
        "CHUTNEY", "COMPOTE",
    ]),
    ("Sauces & condiments", &[
        "SAUCE", "KETCHUP", "MAYONAISE", "MAYONNAISE", "MUSTARD", "DRESSING",
        "SOYA", "SOYASAUCE", "KETJAP", "WORCESTER", "TABASCO", "OYSTER",
        "HOI SIN", "SATE", "SATAY", "SAMBAL", "TARTARE", "BBQ", "BARBECUE",
        "HONEY", "JAM", "CURRY", "ROUX", "BOUILLON", "AROMAT",
    ]),
    ("Spices & seasoning", &[
        "SPICE", "PEPPER", "SALT", "SESAME", "CUMIN", "NUTMEG", "SEASONING",
        "VANILLE", "EXTRACT", "THAI", "MASSALA", "KERRIE", "SPICEMIX",
    ]),
    ("Oils & fats", &["OIL", "FAT", "CROMA", "MARGARINE"]),
    ("Pasta, rice & noodles", &[
        "PASTA", "SPAGHETTI", "MACARONI", "PENNE", "LINGUINE", "TORTELLINI",
        "RAVIOLI", "NOODLE", "RICE", "BASMATI", "TORTILLA", "NAAN", "PITA",
    ]),
    ("Dry goods & baking", &[
        "FLOUR", "SUGAR", "OATMEAL", "CRUESLI", "CEREAL", "MUESLI", "CRACKER",
        "BISCUIT", "COOKIE", "WAFELS", "DIGESTIVE", "LENTILS", "PEANUTS",
        "CACAO", "CAKEMIX", "BATTERMIX", "BREAD CRUMBS", "RAISINS", "MAIZENA",
        "BAKED BEANS",
    ]),
    ("Bakery & bread", &["BREAD", "BUN", "BAGUETTE", "DONUT", "PASTRY", "STROOPWAFEL", "YORKSHIRE"]),
    ("Sweets & snacks", &[
        "CHOCOLATE", "CANDY", "GUMS", "TOBLERONE", "TWIX", "SNICKERS", "BOUNTY",
        "MARS", "CHIPS", "LAY'S", "LAYS", "OREO", "DROP", "LICORICE", "HARLEQUIN",
        "LADY FINGERS",
    ]),
    ("Coffee & tea", &["COFFEE", "NESCAFE", "LAVAZZA", "DOUWE", "TEA BAG", "TEA ", "ROOIBOS", "EARL GREY", "CAMOMILE"]),
    ("Drinks", &[
        "COLA", "PEPSI", "FANTA", "TONIC", "SODA", "ICE TEA", "JUICE", "DRINK",
        "GUAVA", "CRANBERRY", "PEACH", "WINE CHARDONNAY", "SOFT DRINK",
    ]),
    ("Water", &["WATER"]),
    ("Cleaning & cabin", &[
        "WASHINGPOWDER", "WASHINGLIQUID", "SOFTENER", "DISHWASH", "DETERGENT",
        "SOAP", "CLEANER", "DESINFECT", "DESCALER", "ONTSTOPPER", "GARBAGE",
        "BAGS", "FOIL", "PAPER", "NAPKIN", "TOILET", "GLOVES", "SPONGE", "SCRUB",
        "MOP", "DUSTER", "STIRRER", "CUP", "CUTLERY", "SANITAIR", "HYGIENE",
        "BLEACH", "AJAX", "DREFT", "OMO", "ROBIJN", "CLING",
    ]),
    ("Clothing & rags", &["RAGS", "COOK'S JACKET", "CHINO", "OVEN GLOVES"]),
    ("Kitchen equipment", &["PAN", "GRATER", "STRAINER", "GASTRONORM", "WOK", "PIPING TIPES", "CUTLERY TRAY"]),
    ("Transport & services", &["TRANSPORT"]),
];

pub fn categorize(description: &str, invoice_reference: &str) -> String {
    let r = invoice_reference.to_uppercase();
    match r.as_str() {
        "BONDED" => return "Bonded store".to_string(),
        "CABIN" => return "Cleaning & cabin".to_string(),
        "WORKING CLOTHES" | "COTTON RAGS" | "RAGS" => return "Clothing & rags".to_string(),
        _ => {}
    }
    if r == "PROVISIONS" || r.is_empty() {
        let text = description.to_uppercase();
        for (cat, kws) in RULES {
            if *cat == "Cleaning & cabin" || *cat == "Clothing & rags"
                || *cat == "Kitchen equipment" || *cat == "Transport & services"
            {
                continue;
            }
            if kws.iter().any(|kw| text.contains(kw)) {
                return cat.to_string();
            }
        }
        for (cat, kws) in RULES {
            if *cat == "Transport & services" || *cat == "Kitchen equipment" {
                if kws.iter().any(|kw| text.contains(kw)) {
                    return cat.to_string();
                }
            }
        }
    }
    "Other".to_string()
}

pub fn normalize_name(description: &str) -> String {
    description.split_whitespace().collect::<Vec<_>>().join(" ").to_uppercase()
}
