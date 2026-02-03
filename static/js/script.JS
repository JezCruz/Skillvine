document.addEventListener("DOMContentLoaded", () => {
  /* ---------------------------------------------------
     🌿 ELEMENT REFERENCES
  --------------------------------------------------- */
  const heroBtn = document.querySelector(".hero-btn");
  const featuresSection = document.querySelector(".features");

  const navLinks = document.querySelector(".nav-links");
  const menuToggle = document.querySelector(".menu-toggle");
  const loginBtn = document.querySelector("header .btn");

  const homeLink = document.getElementById("home-link");

  /* ---------------------------------------------------
     🌿 HELPERS
  --------------------------------------------------- */
  const closeMobileMenu = () => {
    if (navLinks) navLinks.classList.remove("active");
    if (menuToggle) menuToggle.classList.remove("open"); // optional visual state
  };

  const isMenuOpen = () => navLinks && navLinks.classList.contains("active");

  const openPopup = (tabEl) => {
    // close menu first so popup appears correctly
    closeMobileMenu();
    tabEl.classList.add("show");
  };

  const closeAllPopups = () => {
    document.querySelectorAll(".tab-popup.show").forEach((p) => {
      p.classList.remove("show");
    });
  };

  /* ---------------------------------------------------
     🌿 HERO SCROLL
  --------------------------------------------------- */
  if (heroBtn && featuresSection) {
    heroBtn.addEventListener("click", (e) => {
      e.preventDefault();
      featuresSection.scrollIntoView({ behavior: "smooth" });
      closeMobileMenu();
    });
  }

  /* ---------------------------------------------------
     🌿 POPUPS (ABOUT / SERVICES / CONTACT)
  --------------------------------------------------- */
  const popupLinks = [
    { linkId: "about-link", tabId: "about-tab" },
    { linkId: "services-link", tabId: "services-tab" },
    { linkId: "contact-link", tabId: "contact-tab" },
  ];

  popupLinks.forEach(({ linkId, tabId }) => {
    const link = document.getElementById(linkId);
    const tab = document.getElementById(tabId);
    if (!link || !tab) return;

    link.addEventListener("click", (e) => {
      e.preventDefault();
      openPopup(tab);
    });
  });

  // Close popup via X button
  document.querySelectorAll(".close-tab").forEach((btn) => {
    btn.addEventListener("click", () => {
      const popup = btn.closest(".tab-popup");
      if (popup) popup.classList.remove("show");
    });
  });

  // Close popup when clicking overlay (outside tab-content)
  window.addEventListener("click", (e) => {
    if (e.target.classList && e.target.classList.contains("tab-popup")) {
      e.target.classList.remove("show");
    }
  });

  // ESC to close popup (nice UX)
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") closeAllPopups();
  });

  /* ---------------------------------------------------
     🌿 MOBILE MENU TOGGLE (ONLY .active)
  --------------------------------------------------- */
  if (menuToggle && navLinks) {
    menuToggle.addEventListener("click", (e) => {
      e.stopPropagation(); // prevents immediate "outside click" close
      navLinks.classList.toggle("active");
      menuToggle.classList.toggle("open"); // optional for icon animation
    });

    // Close menu when clicking a nav link (better UX)
    navLinks.querySelectorAll("a").forEach((a) => {
      a.addEventListener("click", () => closeMobileMenu());
    });

    // Close menu when tapping outside
    document.addEventListener("click", (e) => {
      if (!isMenuOpen()) return;
      if (!navLinks.contains(e.target) && !menuToggle.contains(e.target)) {
        closeMobileMenu();
      }
    });

    // Close menu when resizing to desktop
    window.addEventListener("resize", () => {
      if (window.innerWidth > 768) closeMobileMenu();
    });
  }

  /* ---------------------------------------------------
     🌿 LOGIN BUTTON IN HEADER
     (If this is already <a href="/login">, it's safe to not handle click,
      but this keeps it consistent even if you change markup later.)
  --------------------------------------------------- */
  if (loginBtn) {
    loginBtn.addEventListener("click", (e) => {
      // If it's an <a>, let it work normally
      // But if you keep inline styles and want JS redirect, do:
      // e.preventDefault();
      // window.location.href = "/login";
    });
  }

  /* ---------------------------------------------------
     🌿 SCROLL TO TOP (HOME)
  --------------------------------------------------- */
  if (homeLink) {
    homeLink.addEventListener("click", (e) => {
      e.preventDefault();
      window.scrollTo({ top: 0, behavior: "smooth" });
      closeMobileMenu();
      closeAllPopups();
    });
  }

  /* ---------------------------------------------------
     🌿 FORM TOGGLE (OPTIONAL LOGIN/SIGNUP)
     (Works only if those elements exist on the page)
  --------------------------------------------------- */
  const toggleForm = document.getElementById("toggle-form");
  const loginForm = document.getElementById("login-form");
  const signupForm = document.getElementById("signup-form");
  const formTitle = document.getElementById("form-title");
  const formSubtitle = document.getElementById("form-subtitle");

  if (toggleForm && loginForm && signupForm && formTitle && formSubtitle) {
    toggleForm.addEventListener("click", () => {
      loginForm.classList.toggle("hidden");
      signupForm.classList.toggle("hidden");

      if (loginForm.classList.contains("hidden")) {
        formTitle.textContent = "Sign Up for Skillvine";
        formSubtitle.innerHTML =
          'Already have an account? <span class="toggle-form">Login</span>';
      } else {
        formTitle.textContent = "Login to Skillvine";
        formSubtitle.innerHTML =
          'Don\'t have an account? <span class="toggle-form">Sign Up</span>';
      }
    });

    // Event delegation (click the inserted span)
    document.addEventListener("click", (e) => {
      if (e.target && e.target.matches(".toggle-form")) {
        toggleForm.click();
      }
    });
  }
});
