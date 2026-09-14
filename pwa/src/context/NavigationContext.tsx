import React, { createContext, useContext, useState, ReactNode } from 'react';

interface NavigationContextType {
    activePage: string;
    setActivePage: (page: string) => void;
}

const NavigationContext = createContext<NavigationContextType | undefined>(undefined);

export const NavigationProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
    const [activePage, setActivePage] = useState('dashboard');

    return (
        <NavigationContext.Provider value={{ activePage, setActivePage }}>
            {children}
        </NavigationContext.Provider>
    );
};

export const useNavigation = () => {
    const context = useContext(NavigationContext);
    if (context === undefined) {
        throw new Error('useNavigation must be used within a NavigationProvider');
    }
    return context;
};
